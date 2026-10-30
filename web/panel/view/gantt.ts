/**
 * The gantt tab (SPC g g): the whole tree on the left, a date axis on the right, one bar set per row. The values are
 * `report::gantt`'s (`tui --json --deep`'s `gantt`, carried on each row); view/gantt-model.ts puts them on the axis and
 * this file draws them. **The page does not judge a date** — whether a row is late, whether its start is known and
 * where a group's plan came from are moai's answers, drawn as given.
 *
 * Layout: one scroll box holds two columns, the grid (left, `position: sticky`, so it never moves sideways) and the
 * dates (right). They share the one vertical scroll, so a row is the same row in both. Every position is set through
 * the CSSOM (`el.style.left`, `el.style.width`, `style.setProperty`) and every colour by a class, because the page's
 * CSP allows no style attribute; no colour stands alone — each has a glyph, a texture or a letter beside it.
 *
 * Keys are the key router's (panel/keys/): it calls command() — j/k the cursor, H/L the dates, -/= the step, t today,
 * z fit, Enter the detail — and this view listens for none. The mouse is this view's (moai-d86e.jmq, "the mouse"
 * below): a click chooses, the date header drags, Shift+wheel moves the dates, Ctrl+wheel steps the zoom, and the
 * grid's handle sets its width. A bar drags to move the row's planned days ("moving a plan" below, moai-d86e.3ud) — the
 * page's one write to the tracker.
 */
import { assigneeOf } from '../data/people.ts'
import { MEMORY_KEY, settingsStorage } from '../data/memory.ts'
import { undoOf, type PlanChange } from '../data/plan.ts'
import { zoneNow } from '../data/zone.ts'
import { startsFolded, pathKey, type GanttRow, type IssueIndex, type Row } from '../data/index.ts'
import { fill } from '../i18n/index.ts'
import type { State } from '../store.ts'
import { el, emptyState, icon } from './components.ts'
import { segLabel } from './crumbs.ts'
import { formatDate } from './detail-format.ts'
import {
  arrowOf, DAY, DEFAULT_GRID, DEFAULT_ZOOM, dayStart, dayText, dragDays, fitSpan, floorDay, ganttLines, HEAD_H, headerOf, movedPlan, normalizeGrid, placeRow,
  PX_PER_DAY, rangeOf, MIN_GRID, ROW_H, shiftPlan, wallAt, wallOf, weekends, widthOf, xOf, ZOOMS, zoomToFit, type Axis, type Edge, type GanttLine,
  type Placed, type Tick, type Zoom,
} from './gantt-model.ts'
import { firstStatus, glint, spins } from './spin.ts'
import { narrow, revealOffset, statusGlyph, statusOf, twigParts, TWIG_TEXT, windowRange } from './list-model.ts'
import type { MountPane, PaneView } from './types.ts'

/** What the view reads from outside: the clock and the zone dates are drawn in. Tests replace them. */
export const clock = {
  now: (): number => Date.now(),
  zone: (): string | undefined => zoneNow(),
}

/** Where the grid's width is remembered (this browser's, not a project's). */
export const GRID_KEY = `${MEMORY_KEY}.gantt.grid`
/** The visible width assumed while the pane has none (hidden, or not laid out yet). */
const FALLBACK_WIDTH = 900
/** The `‹ ›` marks sit this far inside the dates' visible edge. */
const CLIP_INSET = 2
/** The window drawn around what is in view: this many rows more above and below, and a view's width more either side. */
const OVERSCAN_ROWS = 10
/** The height assumed while the pane has none (hidden, or not laid out yet) — the window's and the keys' page size. */
const FALLBACK_HEIGHT = 720

/**
 * A row the write cannot reach — `moai edit` finds a row in the main checkout's live snapshot only. With the worktree
 * overlay on, a sibling worktree's row (`branch`, which `show --worktree` puts only on those) may not be there at all
 * ("not found"), and where it is, the days drawn are the sibling's, not the ones a move would be measured from. An
 * archived row (`archived`: shown with `SPC v o`, or an archived group still drawn over a reopened member) is not in the
 * snapshot at all, so its move would only ever come back "not found" (moai-vj8w review).
 */
const unwritable = (row: Row): boolean => (typeof row.branch === 'string' && row.branch !== '') || row.archived === true

function loadGrid(): number {
  try {
    return normalizeGrid(settingsStorage.read(GRID_KEY))
  } catch {
    return DEFAULT_GRID
  }
}

function saveGrid(width: number): void {
  try {
    settingsStorage.write(GRID_KEY, String(width))
  } catch {
    // A refused write only forgets.
  }
}

interface Model {
  index: IssueIndex
  key: string
  matched: State['matched']
  boot: State['boot']
  /** The zone the bars were placed in (`SPC o t`): a new one places every actual band and today again. */
  zone: string | undefined
  lines: GanttLine[]
}

interface Built {
  /**
   * Per line number, the grid row and the date lane — **only the lines in (or near) the window** (moai-vj8w.2fh): the
   * rest are not nodes at all, as the list draws only the rows in view.
   */
  rows: Map<number, HTMLElement>
  lanes: Map<number, HTMLElement>
  /** Per drawn line: the `‹` and `›` marks of the lane, hidden until the dates scroll past the row. */
  clips: Map<number, { left: HTMLElement; right: HTMLElement }>
  /** Per line, drawn or not: where its bars stand. */
  placed: Array<Placed | null>
  /**
   * The wall clock `placed` was measured at. Every window draws the today line, its cap and the header's today there
   * too, so a window built later — the clock moved on, no read since — is still one picture: an open bar ends on the
   * line (moai-vj8w review).
   */
  now: number
  /** The header's top ticks, to keep their label in view while the dates scroll. */
  top: Array<{ el: HTMLElement; from: number; to: number }>
  svg: SVGSVGElement
  /** The today line, at `now` on this axis — neither moves for the life of this Built, so every window shows the one node. */
  today: HTMLElement
  /** The weekend blocks of the dates' window: built again only when that window moves, kept while only the rows' does. */
  shade: HTMLElement[]
}

let mounted = 0

export const mountGantt: MountPane = (root, ctx): PaneView => {
  const { document: doc, copy, intents } = ctx
  const words = copy.gantt
  const shut = new Set<string>()
  let foldVersion = 0
  let seeded = false
  const win = doc.defaultView
  const SVG = 'http://www.w3.org/2000/svg'
  const uid = `moai-gantt-${++mounted}`

  let zoom: Zoom = DEFAULT_ZOOM
  let gridW = loadGrid()
  let state: State | null = null
  let model: Model | null = null
  let axis: Axis | null = null
  let built: Built | null = null
  /** The window the nodes were built for: lines `[r0, r1)` and the dates' pixels `[x0, x1]`. */
  let drawn: { r0: number; r1: number; x0: number; x1: number } | null = null
  let positioned = false
  let shownCursor: string | null | undefined
  /** A bar was dragged: the click the release sends is the drag's, not a choice of the row. Every press clears it. */
  let dragged = false
  /**
   * A bar is being dragged. A draw meanwhile — the rows read again after another session's write, the page's own
   * write landing — would rebuild the lanes under the pointer and drop the drag with nothing said, so it waits for
   * the release ({@link resume}): `waiting` is the last render asked for, `drawLater` a draw asked for directly.
   */
  let holding = false
  let waiting: [State, IssueIndex | null] | null = null
  let drawLater = false
  /**
   * A scroll left the window while a bar was held: the window is owed, not a draw. A draw() would place every bar and
   * the axis again; the window only rebuilds what the view left.
   */
  let paintLater = false
  /** Rows whose planned days are being written: a bar drawn from the read before that write does not move again. */
  const writing = new Set<string>()
  /** Dropped plans stay visible through window rebuilds until the write and its refresh finish. */
  const moved = new Map<string, NonNullable<GanttRow['planned']>>()
  const ganttOf = (row: Row): GanttRow | undefined => {
    const planned = moved.get(row.id)
    return planned && row.gantt ? { ...row.gantt, planned } : row.gantt
  }
  /**
   * Where a row's bars stand, a pending move included. placeRow reads a milestone's `◆` off the row's own `due_on`, not
   * off its plan, so the dropped deadline goes onto the row too: else the `◆` went back to its old day on the next
   * window while the bar stayed where it was dropped.
   */
  const placeOf = (row: Row, now: number, zone: string | undefined): Placed => {
    const planned = moved.get(row.id)
    return placeRow(ganttOf(row), planned ? { ...row, due_on: planned.end_filled ? undefined : planned.end } : row, now, zone)
  }

  // ── frame ──
  const zoomButtons = new Map<Zoom, HTMLButtonElement>()
  const seg = el(doc, 'div', { class: 'g-seg', role: 'group', 'aria-label': words.zoomLabel })
  for (const z of [...ZOOMS].reverse()) {
    const b = el(doc, 'button', { type: 'button', 'data-zoom': z, tabindex: -1 }, words.zoom[z])
    b.addEventListener('click', () => keepFocus(() => setZoom(z)))
    zoomButtons.set(z, b)
    seg.append(b)
  }
  const key = (text: string) => el(doc, 'kbd', {}, text)
  const todayButton = el(doc, 'button', { type: 'button', class: 'm-btn-sm', tabindex: -1 }, words.today, ' ', key('t'))
  todayButton.addEventListener('click', () => keepFocus(goToday))
  const fitButton = el(doc, 'button', { type: 'button', class: 'm-btn-sm', tabindex: -1 }, words.fit, ' ', key('z'))
  fitButton.addEventListener('click', () => keepFocus(fit))
  const rangeText = el(doc, 'span', { class: 'g-range' })
  const bar = el(doc, 'div', { class: 'g-toolbar' }, seg, todayButton, fitButton, el(doc, 'span', { class: 'g-spacer' }), rangeText)

  const rowsBox = el(doc, 'div', { class: 'g-rows', role: 'listbox', 'aria-label': words.label })
  const split = el(doc, 'div', { class: 'g-split', role: 'separator', 'aria-orientation': 'vertical', 'aria-label': words.resizer })
  const leftHead = el(doc, 'div', { class: 'g-head' },
    el(doc, 'span', { class: 'c-id' }, words.heads.id), el(doc, 'span', { class: 'c-st' }), el(doc, 'span', { class: 'c-title' }, words.heads.title),
    el(doc, 'span', { class: 'c-n' }, words.heads.progress), el(doc, 'span', { class: 'c-who' }, words.heads.assignee))
  const left = el(doc, 'div', { class: 'g-left' }, split, leftHead, rowsBox)
  const hd1 = el(doc, 'div', { class: 'hd1' })
  const hd2 = el(doc, 'div', { class: 'hd2' })
  const axisBox = el(doc, 'div', { class: 'g-head' }, hd1, hd2)
  const lanesBox = el(doc, 'div', { class: 'g-lanes' })
  const right = el(doc, 'div', { class: 'g-right' }, axisBox, lanesBox)
  const inner = el(doc, 'div', { class: 'g-inner' }, left, right)
  const bubble = el(doc, 'div', { class: 'g-bubble', role: 'tooltip', hidden: true })
  const scroll = el(doc, 'div', { class: 'g-scroll', 'data-keep-scroll': '' }, inner, bubble)
  const empty = el(doc, 'div', { class: 'g-empty', hidden: true })
  const legend = el(doc, 'div', { class: 'g-legend' })
  const pane = el(doc, 'div', { class: 'm-gantt', id: uid, tabindex: -1, 'aria-label': words.label }, bar, scroll, empty, legend)
  root.replaceChildren(pane)
  drawLegend()

  /** A button of this tab that was clicked hands the focus back to the pane, so the keys go on working. */
  function keepFocus(act: () => void): void {
    act()
    pane.focus({ preventScroll: true })
  }

  // ── geometry ──
  const nowWall = (): number => wallOf(clock.now(), clock.zone()) ?? clock.now()
  /** The grid's width as drawn: the remembered one, less than that when the pane is too narrow to leave the dates 120px. */
  const gridNow = (): number => Math.min(gridW, Math.max(MIN_GRID, (scroll.clientWidth || FALLBACK_WIDTH) - 120))
  const viewWidth = (): number => Math.max(120, (scroll.clientWidth || FALLBACK_WIDTH) - gridNow())

  function paintGrid(): void {
    const width = gridNow()
    inner.style.setProperty('--grid-w', `${width}px`)
    // A narrow grid gives up the assignee, then the progress, as the list does as it narrows.
    left.dataset.cols = width < 300 ? 'min' : width < 400 ? 'mid' : 'full'
    split.setAttribute('aria-valuenow', String(width))
  }

  // ── drawing ──
  const bars = (lane: HTMLElement, cls: string, from: number, to: number, a: Axis): HTMLElement => {
    const node = el(doc, 'div', { class: `bar ${cls}` })
    const x0 = xOf(a, from)
    node.style.left = `${x0}px`
    node.style.width = `${Math.max(2, xOf(a, to) - x0)}px`
    lane.append(node)
    return node
  }
  const mark = (lane: HTMLElement, cls: string, text: string, x: number): HTMLElement => {
    const m = el(doc, 'span', { class: `mk ${cls}`, 'aria-hidden': 'true' }, text)
    m.style.left = `${x}px`
    lane.append(m)
    return m
  }

  function twigOf(line: GanttLine): HTMLElement | null {
    const parts = twigParts(line.twig)
    if (!parts.length) return null
    return el(doc, 'span', { class: 'moai-twig', 'aria-hidden': 'true' }, ...parts.map((part) => el(doc, 'span', { class: 'moai-tw', 'data-tw': part }, TWIG_TEXT[part])))
  }

  function gridRow(m: Model, line: GanttLine, g: GanttRow | undefined, cursor: string | null): HTMLElement {
    const row = line.kind === 'row' ? m.index.rows[line.at]! : null
    const status = row ? statusOf(row) : null
    const node = el(doc, 'div', {
      class: `g-row${g?.deferred ? ' dim' : ''}`, role: 'option', 'aria-selected': String(line.key === cursor), 'data-id': row ? row.id : null, 'data-status': status, 'data-key': line.key,
    })
    const title = el(doc, 'span', { class: `c-title${line.dir ? ' grp' : ''}${row?.kind === 'milestone' ? ' ms' : ''}` })
    const twig = twigOf(line)
    if (twig) title.append(twig)
    if (line.dir) {
      const label = line.open ? copy.list.collapse : copy.list.expand
      const toggle = el(doc, 'button', { type: 'button', class: 'moai-toggle', tabindex: -1, 'aria-label': label, title: label, 'aria-expanded': String(line.open) }, icon(doc, 'chevron-right', 12))
      toggle.addEventListener('click', (event) => {
        event.stopPropagation()
        keepFocus(() => fold(line, !line.open))
      })
      toggle.addEventListener('dblclick', (event) => event.stopPropagation())
      title.append(toggle)
    }
    if (row && status !== null) {
      const { glyph, tone } = statusGlyph(status, m.boot)
      const name = String(row.title ?? row.id)
      const moving = line.kind === 'row' && spins(m.index, line.at, firstStatus(m.boot), m.boot?.statuses)
      title.append(row.kind === 'milestone' ? '◆ ' : '', moving ? glint(doc, name) : name)
      title.title = name
      const progress = g?.progress.kind === 'rollup' && g.progress.total > 0 ? `${g.progress.done}/${g.progress.total}` : ''
      const st = el(doc, 'span', { class: `c-st m-st-${tone}${moving ? ' m-spin' : ''}`, title: (copy.stats as Record<string, string>)[status] ?? status }, glyph)
      node.append(el(doc, 'span', { class: 'c-id' }, row.id), st, title, el(doc, 'span', { class: 'c-n' }, progress), el(doc, 'span', { class: 'c-who' }, assigneeOf(row)))
    } else {
      title.append(line.kind === 'bucket' ? segLabel(line.seg, m.index, copy) : '')
      node.append(el(doc, 'span', { class: 'c-id' }), el(doc, 'span', { class: 'c-st' }), title, el(doc, 'span', { class: 'c-n' }), el(doc, 'span', { class: 'c-who' }))
    }
    node.addEventListener('click', () => (row ? intents.choose(line.key) : intents.point(line.key)))
    return node
  }

  function draw(scrollTo?: (a: Axis) => number): void {
    if (!model || !state) return
    if (holding) return void (drawLater = true)
    drawLater = false
    const m = model
    const now = nowWall()
    const zone = clock.zone()
    const placed = m.lines.map((line) => (line.kind === 'row' ? placeOf(m.index.rows[line.at]!, now, zone) : null))
    const was = axis
    if (!axis || axis.zoom !== zoom || !fresh(placed, now)) {
      const [from, to] = rangeOf(placed.map((p) => p?.span ?? null), now)
      axis = { zoom, from, to }
    }
    const a = axis
    // The bars this hovered bubble spoke of are about to be replaced, and a removed node sends no mouseleave.
    bubble.hidden = true
    const wide = widthOf(a)
    right.style.width = `${wide}px`
    lanesBox.style.height = `${m.lines.length * ROW_H}px`
    rowsBox.style.height = `${m.lines.length * ROW_H}px`
    // An axis that grew at its start (a new row reaches earlier) moves every date right: the same dates stay in view.
    // A new step is setZoom's and fit's to place.
    if (scrollTo) scroll.scrollLeft = scrollTo(a)
    else if (was && was.zoom === a.zoom && was.from !== a.from) scroll.scrollLeft = Math.max(0, scroll.scrollLeft + xOf(a, was.from))

    const svg = doc.createElementNS(SVG, 'svg')
    svg.setAttribute('class', 'links')
    svg.setAttribute('width', String(wide))
    svg.setAttribute('height', String(m.lines.length * ROW_H))
    svg.setAttribute('aria-hidden', 'true')
    const today = el(doc, 'div', { class: 'todayline' })
    today.style.left = `${xOf(a, now)}px`
    built = { rows: new Map(), lanes: new Map(), clips: new Map(), placed, now, top: [], svg, today, shade: [] }
    drawn = null
    paintWindow()

    empty.hidden = m.lines.length > 0
    empty.replaceChildren(...(m.lines.length ? [] : [emptyState(doc, { icon: 'gantt-chart', title: words.empty })]))
    for (const [z, b] of zoomButtons) b.setAttribute('aria-pressed', String(z === zoom))
    paintArrows()
    paintView()
  }

  /** What is in view now: the lines `[r0, r1)` and the dates' pixels `[x0, x1]`, with no margin. */
  function inView(): NonNullable<typeof drawn> {
    const total = model ? model.lines.length : 0
    const { start, end } = rowRange(total, 0)
    const x0 = scroll.scrollLeft
    return {
      r0: start,
      r1: end,
      x0,
      x1: x0 + viewWidth(),
    }
  }

  const rowRange = (count: number, overscan: number) => windowRange({
    scrollTop: scroll.scrollTop, viewport: scroll.clientHeight || FALLBACK_HEIGHT,
    offset: HEAD_H, rowHeight: ROW_H, count, overscan,
  })

  /** Whether everything in view stands among the nodes built. */
  function covered(): boolean {
    if (!drawn) return false
    const v = inView()
    return v.r0 >= drawn.r0 && v.r1 <= drawn.r1 && v.x0 >= drawn.x0 && v.x1 <= drawn.x1
  }

  /**
   * Build the nodes of the window: the lines in view and ten more either side, the weekend blocks and header ticks of
   * the dates in view and a view's width more either side, and the bars that reach into those dates. Nothing outside is
   * a node, so a long axis or a long list costs what the screen shows (moai-vj8w.2fh). Drawn again when a scroll leaves it.
   */
  function paintWindow(): void {
    if (!model || !state || !built || !axis) return
    // A held bar keeps its lane: the window is built on release (resume), from the view then — not a whole draw().
    if (holding) return void (paintLater = true)
    paintLater = false
    const m = model
    const a = axis
    const b = built
    // The clock the bars were placed at (draw()), not a new one: the today line stays where the open bars end.
    const now = b.now
    const cursor = state.cursor
    const v = inView()
    const view = Math.max(v.x1 - v.x0, 120)
    const rowsInWindow = rowRange(m.lines.length, OVERSCAN_ROWS)
    const previous = drawn
    // The dates' window stays while the view is inside it: a scroll up or down builds only the rows it brings in.
    const dates = previous && v.x0 >= previous.x0 && v.x1 <= previous.x1 ? previous : { x0: v.x0 - view, x1: v.x1 + view }
    const datesChanged = dates !== previous
    const oldRows = b.rows, oldLanes = b.lanes, oldClips = b.clips
    const win = {
      r0: rowsInWindow.start,
      r1: rowsInWindow.end,
      x0: Math.max(0, dates.x0),
      x1: Math.min(widthOf(a), dates.x1),
    }
    // The dates' margin is kept unclamped: a view at the axis' edge is covered by a window that ends there.
    drawn = { ...win, x0: dates.x0, x1: dates.x1 }
    // A bubble over a bar about to go has no mouseleave to take it down.
    bubble.hidden = true
    const wall: [number, number] = [wallAt(a, win.x0), wallAt(a, win.x1)]

    // Weekends, behind everything. Not at the month step, where a day is 4px.
    if (datesChanged) {
      b.shade = []
      if (a.zoom !== 'month') {
        for (const [from, to] of weekends(a, wall)) {
          const col = el(doc, 'div', { class: 'col-we' })
          col.style.left = `${xOf(a, from)}px`
          col.style.width = `${xOf(a, to) - xOf(a, from)}px`
          b.shade.push(col)
        }
      }
    }
    b.rows = new Map()
    b.lanes = new Map()
    b.clips = new Map()
    const rows: HTMLElement[] = []
    const lanes: HTMLElement[] = []
    for (let n = win.r0; n < win.r1; n++) {
      const line = m.lines[n]!
      const g = line.kind === 'row' ? ganttOf(m.index.rows[line.at]!) : undefined
      const p = b.placed[n] ?? null
      const grid = oldRows.get(n) ?? gridRow(m, line, g, cursor)
      grid.style.top = `${n * ROW_H}px`
      // Only the window's options are in the listbox: each says where it stands in the whole set, as the list's rows do
      // with aria-rowindex — or a screen reader counts "3 of 41" in a tree of 400.
      grid.setAttribute('aria-setsize', String(m.lines.length))
      grid.setAttribute('aria-posinset', String(n + 1))
      rows.push(grid)
      b.rows.set(n, grid)
      // A lane is kept while its dates are — but not one whose plan was just dropped (`moved`): its bubble and drag were
      // wired to the plan before the drop, and a window built anew builds it from the dropped one, as it always did.
      const dropped = line.kind === 'row' && moved.has(m.index.rows[line.at]!.id)
      const retained = !datesChanged && !dropped ? oldLanes.get(n) : undefined
      if (retained) {
        lanes.push(retained)
        b.lanes.set(n, retained)
        const clips = oldClips.get(n)
        if (clips) b.clips.set(n, clips)
        continue
      }
      const lane = el(doc, 'div', { class: `lane${g?.deferred ? ' dim' : ''}`, 'data-id': line.kind === 'row' ? m.index.rows[line.at]!.id : null, 'data-key': line.key, 'aria-selected': String(line.key === cursor) })
      lane.style.top = `${n * ROW_H}px`
      lane.addEventListener('click', () => {
        // The click that ends a bar's drag is the drag's, not a choice.
        if (dragged) return void (dragged = false)
        if (line.kind === 'row') intents.choose(line.key)
        else intents.point(line.key)
      })
      if (line.kind === 'row') plannable(lane, m.index.rows[line.at]!, g)
      if (p && g) {
        // A row whose dates all lie outside the window draws no bars: there is nothing of them to see.
        if (p.span && xOf(a, p.span[1]) >= win.x0 && xOf(a, p.span[0]) <= win.x1) drawBars(lane, m, line, g, p, a)
        const l = mark(lane, 'clip', '‹', 0)
        const r = mark(lane, 'clip', '›', 0)
        l.hidden = r.hidden = true
        if (p.span) b.clips.set(n, { left: l, right: r })
      }
      lanes.push(lane)
      b.lanes.set(n, lane)
    }
    lanesBox.replaceChildren(...b.shade, ...lanes, b.svg, b.today)
    rowsBox.replaceChildren(...rows)

    // The date decorations remain when only the vertical window changes.
    if (!datesChanged) return
    // The header; its today cap stands over the day's line.
    b.top = []
    const header = headerOf(a, now, words, wall)
    const ticks = (box: HTMLElement, list: Tick[], keep: boolean) => {
      box.replaceChildren(...list.map((tick) => {
        const node = el(doc, 'div', { class: `tick${tick.cls ? ` ${tick.cls}` : ''}` }, tick.text)
        node.style.left = `${xOf(a, tick.from)}px`
        node.style.width = `${xOf(a, tick.to) - xOf(a, tick.from)}px`
        if (keep) b.top.push({ el: node, from: xOf(a, tick.from), to: xOf(a, tick.to) })
        return node
      }))
    }
    ticks(hd1, header.top, true)
    ticks(hd2, header.bottom, false)
    const cap = el(doc, 'div', { class: 'todaycap', 'aria-hidden': 'true' })
    cap.style.left = `${xOf(a, now)}px`
    hd1.append(cap)
  }

  /** The bars of one row: planned, actual, late, the `◆`, and what drags. */
  function drawBars(lane: HTMLElement, m: Model, line: GanttLine, g: GanttRow, p: Placed, a: Axis): void {
    const hover = (node: HTMLElement) => tip(node, m, line, g)
    let planBar: HTMLElement | null = null
    let dueMark: HTMLElement | null = null
    if (p.plan) {
      const plan = (planBar = bars(lane, `b-plan${p.plan.origin === 'members' ? ' members' : ''}${g.deferred ? ' deferred' : ''}`, p.plan.from, p.plan.to, a))
      // A group's plan carries how far its members are, as a fill from the left.
      if (line.dir && g.progress.kind === 'rollup' && g.progress.total > 0) {
        const fillBar = el(doc, 'div', { class: 'b-fill' })
        fillBar.style.width = `${(g.progress.done / g.progress.total) * 100}%`
        plan.append(fillBar)
      }
      hover(plan)
    }
    if (p.actual) hover(bars(lane, `b-act a-${statusGlyph(p.status, m.boot).tone}`, p.actual.from, p.actual.to, a))
    if (p.over) {
      hover(bars(lane, 'b-over', p.over.from, p.over.to, a))
      mark(lane, 'over', '!', xOf(a, p.over.to) + 3)
    }
    if (p.due !== null) hover((dueMark = mark(lane, 'due', '◆', xOf(a, p.due))))
    if (line.kind === 'row') movable(lane, m.index.rows[line.at]!, g, planBar, dueMark)
  }

  /** The axis stands while the lines' dates still fit in it; a draw because the index was read again does not shift it. */
  function fresh(placed: Array<Placed | null>, now: number): boolean {
    if (!axis) return false
    const [from, to] = rangeOf(placed.map((p) => p?.span ?? null), now)
    return from >= axis.from && to <= axis.to
  }

  /** Only links touching the cursor, in blocker → blocked direction, with the same style both ways. */
  function paintArrows(): void {
    if (!built || !model || !axis || !state) return
    const svg = built.svg
    svg.replaceChildren()
    const focus = model.lines.findIndex((line) => line.key === state!.cursor)
    if (focus < 0) return
    const visible = new Map<string, number>()
    model.lines.forEach((line, n) => { if (!visible.has(line.key)) visible.set(line.key, n) })
    const edges = new Set<string>()
    for (let to = 0; to < model.lines.length; to++) {
      const line = model.lines[to]!
      if (line.kind !== 'row') continue
      for (const block of model.index.rows[line.at]!.gantt?.blocked_by ?? []) {
        const from = visible.get(block.id) ?? -1
        if (from < 0 || from === to || (from !== focus && to !== focus)) continue
        const key = `${from}:${to}`
        if (edges.has(key)) continue
        edges.add(key)
        const source = built.placed[from]
        const target = built.placed[to]
        const x0 = source?.plan?.to ?? source?.span?.[1]
        const x1 = target?.plan?.from ?? target?.span?.[0]
        if (x0 === undefined || x1 === undefined) continue
        const { d, head } = arrowOf(xOf(axis, x0), from, xOf(axis, x1), to)
        const path = doc.createElementNS(SVG, 'path')
        path.setAttribute('d', d)
        const tipHead = doc.createElementNS(SVG, 'polygon')
        tipHead.setAttribute('points', head)
        svg.append(path, tipHead)
      }
    }
  }

  /** What depends on the scroll offset: the `‹ ›` marks, the header labels kept in view, the range. */
  function paintView(): void {
    if (!built || !axis) return
    // A scroll, a resize or a step that left the window builds the one around where the view is now.
    if (!covered()) paintWindow()
    const v0 = scroll.scrollLeft
    const v1 = v0 + viewWidth()
    built.clips.forEach((pair, n) => {
      const span = built!.placed[n]?.span
      if (!pair || !span) return
      const hideLeft = xOf(axis!, span[0]) >= v0 - 1
      const hideRight = xOf(axis!, span[1]) <= v1 + 1
      pair.left.hidden = hideLeft
      pair.right.hidden = hideRight
      if (!hideLeft) pair.left.style.left = `${v0 + CLIP_INSET}px`
      if (!hideRight) pair.right.style.left = `${v1 - 12}px`
    })
    for (const tick of built.top) tick.el.style.paddingLeft = tick.from < v0 && tick.to > v0 ? `${v0 - tick.from + 4}px` : ''
    rangeText.textContent = `${dayText(wallAt(axis, v0))} ~ ${dayText(wallAt(axis, v1))}`
  }

  /** Only the cursor moved: the selection mark, the arrows, the row in view. */
  function paintCursor(): void {
    if (!built || !model || !state) return
    const cursor = state.cursor
    for (const [n, lane] of built.lanes) {
      const on = String(model.lines[n]!.key === cursor)
      for (const node of [built.rows.get(n)!, lane]) if (node.getAttribute('aria-selected') !== on) node.setAttribute('aria-selected', on)
    }
    paintArrows()
    reveal()
  }

  function reveal(): void {
    if (!model || !state || state.cursor === null) return
    const at = model.lines.findIndex((line) => line.key === state!.cursor)
    const height = scroll.clientHeight
    if (at < 0 || !height) return
    const next = revealOffset(scroll.scrollTop, height, HEAD_H + at * ROW_H, ROW_H, HEAD_H)
    if (next !== scroll.scrollTop) scroll.scrollTop = next
    // The cursor may have gone to a line the window has not reached (the scroll event comes later): build around the view now.
    if (!covered()) paintView()
  }

  // ── the bubble ──
  function tip(node: HTMLElement, m: Model, line: GanttLine, g: GanttRow): void {
    node.addEventListener('mouseenter', () => {
      if (line.kind !== 'row') return
      const row: Row = m.index.rows[line.at]!
      const { glyph } = statusGlyph(statusOf(row), m.boot)
      const dl = el(doc, 'dl')
      const add = (name: string, value: string) => dl.append(el(doc, 'dt', {}, name), el(doc, 'dd', {}, value))
      const zone = clock.zone()
      const t = words.tip
      // A filled edge is drawn but nobody planned it: the bubble says so with `—`, as before moai filled it.
      if (g.planned) add(g.planned.from === 'members' ? t.plannedMembers : t.planned, `${g.planned.start_filled ? '—' : g.planned.start} ~ ${g.planned.end_filled ? '—' : g.planned.end}`)
      add(t.actual, g.actual.kind === 'span' ? `${formatDate(g.actual.start, zone)} ~ ${g.actual.end ? formatDate(g.actual.end, zone) : t.now}` : g.actual.kind === 'unknown' ? t.unknown : t.notStarted)
      if (g.overdue !== null) add(t.overdue, fill(g.overdue === 1 ? t.overdueDay : t.overdueDays, { n: g.overdue }))
      if (g.progress.kind === 'rollup' && g.progress.total > 0) add(t.progress, `${g.progress.done}/${g.progress.total}`)
      if (g.blocked_by.length) add(t.blockedBy, g.blocked_by.map((block) => `${block.id} (${block.state})`).join(', '))
      if (g.deferred) add(t.deferred, t.deferredNote)
      const who = assigneeOf(row)
      if (who) add(t.assignee, who)
      bubble.replaceChildren(el(doc, 'div', { class: 't' }, `${glyph} ${row.id}  ${row.title ?? ''}`), dl)
      bubble.hidden = false
    })
    node.addEventListener('mousemove', (event) => {
      const box = scroll.getBoundingClientRect()
      bubble.style.left = `${event.clientX - box.left + scroll.scrollLeft + 14}px`
      bubble.style.top = `${event.clientY - box.top + scroll.scrollTop + 16}px`
    })
    node.addEventListener('mouseleave', () => {
      bubble.hidden = true
    })
  }

  function drawLegend(): void {
    const l = words.legend
    const item = (text: string, build: (swatch: HTMLElement) => void) => {
      const swatch = el(doc, 'span', { class: 'sw', 'aria-hidden': 'true' })
      build(swatch)
      legend.append(el(doc, 'span', { class: 'g-key' }, swatch, text))
    }
    const sample = (cls: string, width = 26) => (swatch: HTMLElement) => {
      const b = el(doc, 'div', { class: `bar ${cls}` })
      b.style.left = '0px'
      b.style.width = `${width}px`
      swatch.append(b)
    }
    item(l.planned, sample('b-plan'))
    item(l.members, sample('b-plan members'))
    item(l.deferred, sample('b-plan deferred'))
    item(l.actual, sample('b-act a-done'))
    item(l.overdue, sample('b-over'))
    item(l.due, (swatch) => swatch.append(el(doc, 'span', { class: 'mk due' }, '◆')))
    item(l.today, (swatch) => swatch.append(el(doc, 'div', { class: 'todayline' })))
    item(l.clipped, (swatch) => swatch.append(el(doc, 'span', { class: 'mk clip' }, '‹ ›')))
    item(l.blocker, (swatch) => swatch.append(el(doc, 'span', { class: 'arrow-key' }, '└▶')))
  }

  // ── moving about ──
  /** `anchor` is how far from the dates' visible left edge the date to keep still stands (the middle when left out). */
  function setZoom(next: Zoom, anchor?: number): void {
    // A bar being dragged measures its days at the step it was pressed at.
    if (!axis || next === zoom || holding) return
    const view = viewWidth()
    const held = anchor ?? view / 2
    const centre = wallAt(axis, scroll.scrollLeft + held)
    zoom = next
    axis = { ...axis, zoom }
    draw((a) => Math.max(0, xOf(a, centre) - held))
  }

  function goToday(): void {
    if (!axis) return
    scroll.scrollLeft = Math.max(0, xOf(axis, nowWall()) - viewWidth() / 3)
    paintView()
  }

  function fit(): void {
    if (!axis || !built || holding) return
    const span = fitSpan(built.placed.map((p) => p?.span ?? null))
    if (!span) return goToday()
    const next = zoomToFit(span, viewWidth())
    if (next !== zoom) {
      zoom = next
      axis = { ...axis, zoom }
      draw((a) => Math.max(0, xOf(a, span[0]) - 8))
    } else {
      scroll.scrollLeft = Math.max(0, xOf(axis, span[0]) - 8)
      paintView()
    }
  }

  /** The first time the tab shows, the dates start at today; a hidden pane has no width to place them by. */
  function position(): void {
    if (positioned || !axis || !scroll.clientWidth) return
    positioned = true
    goToday()
  }

  const step = (): number => Math.max(PX_PER_DAY[zoom], 40) * 2

  function point(delta: number | 'top' | 'bottom'): void {
    if (!model) return
    const lines = model.lines
    if (!lines.length) return
    const at = state?.cursor == null ? -1 : lines.findIndex((line) => line.key === state!.cursor)
    const to = delta === 'top' ? 0 : delta === 'bottom' ? lines.length - 1 : at < 0 ? (delta < 0 ? lines.length - 1 : 0) : Math.max(0, Math.min(lines.length - 1, at + delta))
    intents.point(lines[to]!.key)
  }

  /** Fold state belongs to this visit, never to the list's expanded folders. */
  function fold(line: GanttLine, open: boolean, all = false): void {
    if (!model || !state || !line.dir) return
    const index = model.index
    const paths = [{ path: line.path, at: line.kind === 'row' ? line.at : undefined }, ...(all ? index.tree(line.path, () => true).filter((one) => one.entry.dir).map((one) => ({ path: one.path, at: one.entry.at })) : [])]
    for (const { path, at } of paths) {
      const key = pathKey(path)
      if (!open) shut.add(key)
      else if (key === pathKey(line.path) || at === undefined || !startsFolded(index.rows[at]!)) shut.delete(key)
    }
    if (!open) {
      const cursor = model.lines.find((one) => one.key === state!.cursor)
      if (cursor && cursor.path.length > line.path.length && line.path.every((seg, n) => cursor.path[n] === seg)) {
        state = { ...state, cursor: line.key }
        intents.point(line.key)
      }
    }
    foldVersion++
    render(state, index)
  }

  function command(action: string): boolean {
    if (!model || !state) return false
    const page = Math.max(1, Math.floor((scroll.clientHeight || FALLBACK_HEIGHT) / ROW_H) - 2)
    const at = model.lines.findIndex((one) => one.key === state!.cursor)
    const line = model.lines[at]
    switch (action) {
      case 'expand':
        if (line?.dir && !line.open) fold(line, true)
        return true
      case 'collapse': {
        if (!line) return true
        if (line.dir && line.open) fold(line, false)
        else {
          const parent = model.lines.slice(0, at).reverse().find((one) => one.dir && one.depth < line.depth)
          if (parent) {
            state = { ...state, cursor: parent.key }
            intents.point(parent.key)
            fold(parent, false)
          }
        }
        return true
      }
      case 'expandAll':
        if (line?.dir) fold(line, !line.open, true)
        return true
      case 'move.down':
      case 'move.up':
      case 'move.pageDown':
      case 'move.pageUp':
      case 'move.top':
      case 'move.bottom':
        point({ 'move.down': 1, 'move.up': -1, 'move.pageDown': page, 'move.pageUp': -page, 'move.top': 'top', 'move.bottom': 'bottom' }[action] as number | 'top' | 'bottom')
        return true
      case 'move.halfDown':
      case 'move.halfUp': {
        const half = Math.max(ROW_H, Math.floor((scroll.clientHeight || FALLBACK_HEIGHT) / 2))
        scroll.scrollTop = Math.max(0, scroll.scrollTop + (action === 'move.halfDown' ? half : -half))
        return true
      }
      case 'enter': {
        const line = model.lines.find((one) => one.key === state!.cursor)
        if (!line) return true
        if (line.kind === 'row') {
          intents.choose(line.key)
          if (ctx.layout().detailVisible) ctx.focusPane('detail')
        }
        return true
      }
      case 'gantt.left':
      case 'gantt.right':
        scroll.scrollLeft = Math.max(0, scroll.scrollLeft + (action === 'gantt.left' ? -step() : step()))
        paintView()
        return true
      case 'gantt.zoomIn':
        setZoom(ZOOMS[Math.min(ZOOMS.length - 1, ZOOMS.indexOf(zoom) + 1)]!)
        return true
      case 'gantt.zoomOut':
        setZoom(ZOOMS[Math.max(0, ZOOMS.indexOf(zoom) - 1)]!)
        return true
      case 'gantt.today':
        goToday()
        return true
      case 'gantt.fit':
        fit()
        return true
    }
    return false
  }

  // ── the mouse ──
  /**
   * A drag of the primary button on `target`, as view/split.ts does it: the pointer is captured, the move and the end
   * listeners stand only while it lasts, and a cancel or a lost capture ends it like a release. `begin` may turn the
   * press down by answering false — it is then an ordinary press, and its click goes on as any other. `end` is handed
   * the event that ended it (a bar lands only on `pointerup`) and the press.
   */
  function drag(
    target: HTMLElement,
    begin: (down: PointerEvent) => boolean | void,
    move: (event: PointerEvent, from: PointerEvent) => void,
    end?: (event: PointerEvent, from: PointerEvent) => void,
  ): void {
    target.addEventListener('pointerdown', (down) => {
      if (down.button !== 0 || begin(down) === false) return
      down.preventDefault()
      target.setPointerCapture?.(down.pointerId)
      target.classList.add('drag')
      const onMove = (event: PointerEvent) => move(event, down)
      const stop = (event: PointerEvent) => {
        target.removeEventListener('pointermove', onMove)
        target.removeEventListener('pointerup', stop)
        target.removeEventListener('pointercancel', stop)
        target.removeEventListener('lostpointercapture', stop)
        target.classList.remove('drag')
        end?.(event, down)
      }
      target.addEventListener('pointermove', onMove)
      target.addEventListener('pointerup', stop)
      target.addEventListener('pointercancel', stop)
      target.addEventListener('lostpointercapture', stop)
    })
  }

  // ── moving a plan (moai-d86e.3ud) ──
  // A bar drags as the TUI's does (moai-i4hf.exq): the middle moves the plan, either end the start or the deadline
  // alone, by whole days; the bar is only drawn while it moves and the row is written once, on release. What is
  // written is the move — the edge and the days — and moai measures the days inside its lock from the row as it stands
  // then (data/plan.ts, moai-vj8w.02c); shiftPlan (gantt-model.ts, `Issue::shifted`'s copy) only draws the bar meanwhile.
  // A refusal — a start past the deadline, a day past the deadline above — comes back in moai's words.

  /** How close to a bar's end a press takes that end rather than the whole bar, in px. */
  const GRIP = 6
  const edgeAt = (box: DOMRect, x: number): Edge => {
    const grip = Math.min(GRIP, box.width / 3)
    return x < box.left + grip ? 'start' : x > box.right - grip ? 'end' : 'body'
  }

  /**
   * The drag let go: what waited for it is drawn — in a task of its own, so the click the release sends still reaches
   * the lane it pressed (a choice made by a plain press must not be lost to a redraw).
   */
  function resume(): void {
    holding = false
    if (!waiting && !drawLater && !paintLater) return
    setTimeout(() => {
      // Another drag took the lanes meanwhile: its release draws.
      if (holding) return
      const next = waiting
      waiting = null
      if (next) render(...next)
      if (drawLater) draw()
      // Only the window was owed (a scroll during the drag): nothing is built if the view came back inside it.
      else if (paintLater) {
        paintLater = false
        paintView()
      }
    }, 0)
  }

  /**
   * Write a row's planned days. One write at a time per row: until it is answered (and the rows read again), the
   * bar on screen was drawn from the read before it — a second drag would be drawn, and its undo line built, from days
   * the first has already moved, while moai adds the second move on top of the first.
   */
  function write(movePlan: NonNullable<typeof ctx.movePlan>, id: string, change: PlanChange, undo: string): void {
    writing.add(id)
    void movePlan(id, change, undo).finally(() => {
      writing.delete(id)
      moved.delete(id)
      // Even a refresh drew the overlay: remove it to show the days moai measured inside its lock.
      draw()
    })
  }

  /** A press on the row's own planned bar, or on its `◆`, drags it. The lane takes the press: the actual band lies over the bar. */
  function movable(lane: HTMLElement, row: Row, g: GanttRow, planBar: HTMLElement | null, dueMark: HTMLElement | null): void {
    const movePlan = ctx.movePlan
    const grab = planBar ?? dueMark
    if (!movePlan || !grab || !axis || unwritable(row)) return
    // A bar or `◆` drawn from a day of the row's own that the plan carries no day for (a milestone's deadline the plan
    // did not read) has nothing to move: it is not grabbed, rather than refused on release.
    const days = g.planned && (dayStart(g.planned.start) !== null || dayStart(g.planned.end) !== null)
    if (!days) return
    const a = axis
    const created = typeof row.created_at === 'string' ? wallOf(row.created_at, clock.zone()) : null
    const made = created === null ? null : floorDay(created)
    // A group's bar counted from its members does not move: the press is still taken, and the release says why.
    const own = g.planned?.from !== 'members'
    if (own) grab.classList.add('movable')
    // A milestone draws its own `◆` at the end of its bar: the mark moves with the bar's end.
    const drawnAt = planBar && dueMark ? [planBar, dueMark] : [grab]
    lane.addEventListener('pointermove', (event) => {
      if (event.buttons || !own) return
      const box = grab.getBoundingClientRect()
      const inside = event.clientX >= box.left - 2 && event.clientX <= box.right + 2
      const grip = inside ? (planBar ? edgeAt(box, event.clientX) : 'body') : ''
      if (lane.dataset.grip !== grip) lane.dataset.grip = grip
    })
    lane.addEventListener('pointerleave', () => void (lane.dataset.grip = ''))
    let edge: Edge = 'body'
    let was: Array<{ left: string; width: string }> = []
    const putBack = () => drawnAt.forEach((node, n) => Object.assign(node.style, was[n]))
    drag(lane, (down) => {
      const box = grab.getBoundingClientRect()
      // A press beside the bar is the lane's, a choice; so is one while another drag, or this row's write, runs.
      if (holding || writing.has(row.id) || down.clientX < box.left - 2 || down.clientX > box.right + 2) return false
      holding = true
      edge = planBar ? edgeAt(box, down.clientX) : 'body'
      was = drawnAt.map((node) => ({ left: node.style.left, width: node.style.width }))
    }, (event, down) => {
      const change = shiftPlan(g.planned, edge, dragDays(a.zoom, event.clientX - down.clientX))
      const span = 'why' in change ? null : movedPlan(g.planned, change, made)
      if (!span) return putBack()
      // The bar spans its days; a `◆` stands at the end of its day.
      if (planBar) {
        planBar.style.left = `${xOf(a, span[0])}px`
        planBar.style.width = `${Math.max(2, xOf(a, span[1]) - xOf(a, span[0]))}px`
      }
      if (dueMark) dueMark.style.left = `${xOf(a, span[1])}px`
    }, (end, down) => {
      // The release is where the bar lands; a cancel or a lost capture puts it back and writes nothing.
      const days = end.type === 'pointerup' ? dragDays(a.zoom, end.clientX - down.clientX) : 0
      const change = shiftPlan(g.planned, edge, days)
      if ('why' in change) {
        putBack()
        // Not a day moved: the press was a click, and it chooses. A day or more is the drag's, and says why it stands.
        if (change.why !== 'still') {
          dragged = true
          ctx.say?.(fill(change.why === 'members' ? words.move.members : words.move.none, { id: row.id }), true)
        }
      } else {
        dragged = true
        const preview = movedPlan(g.planned, change, made)
        if (preview) moved.set(row.id, {
          ...g.planned!, start: dayText(preview[0]), end: dayText(preview[1] - DAY),
          start_filled: change.start === undefined && g.planned!.start_filled,
          end_filled: change.due === undefined && g.planned!.end_filled, from: 'own',
        })
        const n = model?.lines.findIndex((line) => line.key === row.id) ?? -1
        if (built && n >= 0) built.placed[n] = placeOf(row, built.now, clock.zone())
        // The arrows into and out of the dropped bar are drawn from those days.
        paintArrows()
        // Invert the move rather than restoring the page's stale dates; the other edge stays as moai found it.
        // The tree plan is the same picture shiftPlan used; the separately read list may disagree about absent edges.
        const writtenEdge = edge === 'body' ? (g.planned!.start_filled ? 'end' : g.planned!.end_filled ? 'start' : edge) : edge
        const intent: PlanChange = edge === 'start' && g.planned!.start_filled
          ? { start: change.start! } : { edge: writtenEdge, days }
        write(movePlan, row.id, intent, undoOf(row.id, intent, {
          start: g.planned?.start_filled ? null : g.planned?.start ?? null,
          due: g.planned?.end_filled ? null : g.planned?.end ?? null,
        }))
      }
      resume()
    })
  }

  /**
   * A double click on a day of a row that wrote no day plans that one day, as a click does in the TUI. A double click,
   * not a click: here a click on a lane chooses the row, and choosing must never write. A group's bar is its members'.
   */
  function plannable(lane: HTMLElement, row: Row, g: GanttRow | undefined): void {
    const movePlan = ctx.movePlan
    const written = typeof row.starts_on === 'string' || typeof row.due_on === 'string'
    if (!movePlan || !axis || written || unwritable(row) || row.kind === 'epic' || row.kind === 'milestone' || g?.planned) return
    const a = axis
    lane.addEventListener('dblclick', (event) => {
      // A second double click before the first is answered would only be refused: the first gave the row its day, and
      // `--if-unplanned` writes nothing over it.
      if (writing.has(row.id)) return
      const day = dayText(floorDay(wallAt(a, event.clientX - lane.getBoundingClientRect().left)))
      write(movePlan, row.id, { day }, undoOf(row.id, { day }, { start: null, due: null }))
    })
  }

  // Every press on the dates starts afresh: a drag whose release sent no click (a touch, a pen) must not take the
  // click of the next press, on whichever lane it lands.
  lanesBox.addEventListener('pointerdown', () => void (dragged = false), true)

  // The grid's width: dragged by the handle, kept for the next time.
  {
    let start = gridW
    drag(split, () => {
      // From the width drawn, not the one remembered: a narrow pane draws less, and the handle stands there.
      start = gridNow()
    }, (event, down) => {
      gridW = normalizeGrid(start + event.clientX - down.clientX, (scroll.clientWidth || FALLBACK_WIDTH) - 120)
      paintGrid()
      paintView()
    }, () => saveGrid(gridW))
  }

  // The date header: dragging it moves the dates the other way, as a hand on paper.
  {
    let start = 0
    drag(axisBox, () => {
      start = scroll.scrollLeft
    }, (event, down) => {
      scroll.scrollLeft = Math.max(0, start - (event.clientX - down.clientX))
      paintView()
    })
  }

  // The wheel: up and down is the browser's. Shift+wheel moves the dates (a sideways wheel or swipe scrolls the box
  // natively, and the grid is sticky, so only the dates move); Ctrl+wheel steps the zoom and is taken only over the
  // dates, so the page's own zoom stays the browser's everywhere else. The date under the pointer stays under it.
  right.addEventListener('wheel', (event) => {
    if (event.ctrlKey) {
      event.preventDefault()
      // A sideways Ctrl+wheel says nothing about the step.
      if (!event.deltaY) return
      const next = ZOOMS[ZOOMS.indexOf(zoom) + (event.deltaY < 0 ? 1 : -1)]
      if (next) setZoom(next, event.clientX - scroll.getBoundingClientRect().left - gridNow())
      return
    }
    if (event.shiftKey && !event.deltaX) {
      event.preventDefault()
      scroll.scrollLeft = Math.max(0, scroll.scrollLeft + event.deltaY)
      paintView()
    }
  }, { passive: false })

  paintGrid()
  let frame = 0
  const onScroll = () => {
    if (!win?.requestAnimationFrame) return paintView()
    win.cancelAnimationFrame(frame)
    frame = win.requestAnimationFrame(paintView)
  }
  scroll.addEventListener('scroll', onScroll, { passive: true })
  const observer = win && 'ResizeObserver' in win ? new win.ResizeObserver(() => (paintGrid(), position(), paintView())) : null
  observer?.observe(scroll)

  function render(current: State, index: IssueIndex | null): void {
    // A bar being dragged holds the lanes it moves: the newest render waits for its release (resume).
    if (holding) return void (waiting = [current, index])
    waiting = null
    state = current
    if (!index) {
      model = null
      built = null
      rowsBox.replaceChildren()
      lanesBox.replaceChildren()
      return
    }
    if (!seeded) {
      const at = current.cursor === null ? undefined : index.find(current.cursor)
      const selected = at === undefined ? [] : index.homeOf(at)
      for (let n = 0; n < index.rows.length; n++) {
        if (!index.isDir(n) || !startsFolded(index.rows[n]!)) continue
        const path = index.dirPath(n)
        if (path.every((seg, k) => selected[k] === seg)) continue
        shut.add(pathKey(path))
      }
      seeded = true
      foldVersion++
    }
    const key = JSON.stringify([current.path, current.view, current.search.text, current.filter.applied, foldVersion])
    const zone = clock.zone()
    if (!model || model.index !== index || model.key !== key || model.matched !== current.matched || model.boot !== current.boot || model.zone !== zone) {
      const found = narrow(current, index)
      model = { index, key, matched: current.matched, boot: current.boot, zone, lines: ganttLines(index, current.path, found.keep, shut) }
      draw()
      position()
      shownCursor = current.cursor
      reveal()
    } else if (current.cursor !== shownCursor) {
      shownCursor = current.cursor
      paintCursor()
    }
  }

  return {
    render,
    command,
    leave() {
      shut.clear()
      foldVersion++
      seeded = false
      model = null
    },
    focus() {
      pane.focus({ preventScroll: true })
    },
    destroy() {
      observer?.disconnect()
      scroll.removeEventListener('scroll', onScroll)
      pane.remove()
    },
  }
}
