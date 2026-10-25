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
import { undoLine, type PlanChange } from '../data/plan.ts'
import { zoneNow } from '../data/zone.ts'
import type { GanttRow, IssueIndex, Row } from '../data/index.ts'
import { fill } from '../i18n/index.ts'
import type { State } from '../store.ts'
import { el, emptyState } from './components.ts'
import { segLabel } from './crumbs.ts'
import { formatDate } from './detail-format.ts'
import {
  arrowOf, DEFAULT_GRID, DEFAULT_ZOOM, dayStart, dayText, dragDays, fitSpan, floorDay, ganttLines, HEAD_H, headerOf, movedPlan, normalizeGrid, placeRow,
  PX_PER_DAY, rangeOf, MIN_GRID, ROW_H, shiftPlan, wallAt, wallOf, weekends, widthOf, xOf, ZOOMS, zoomToFit, type Axis, type Edge, type GanttLine,
  type Placed, type Tick, type Zoom,
} from './gantt-model.ts'
import { narrow, revealOffset, statusGlyph, statusOf, twigParts, TWIG_TEXT } from './list-model.ts'
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

/** A field the row may carry as text: the text, or null. */
const text = (value: unknown): string | null => (typeof value === 'string' ? value : null)

/**
 * A row the write cannot reach: the worktree overlay is on and the row shown is a sibling worktree's (`branch`, which
 * `show --worktree` puts only on those). The write goes to the main checkout, which may not hold the row at all
 * ("not found"), and where it does hold it the days drawn are the sibling's, not the ones a move would be measured from.
 */
const overlaid = (row: Row): boolean => typeof row.branch === 'string' && row.branch !== ''

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
  /** Per line: the grid row and the date lane. */
  rows: HTMLElement[]
  lanes: HTMLElement[]
  /** Per line: the `‹` and `›` marks of the lane, hidden until the dates scroll past the row. */
  clips: Array<{ left: HTMLElement; right: HTMLElement } | null>
  placed: Array<Placed | null>
  /** The header's top ticks, to keep their label in view while the dates scroll. */
  top: Array<{ el: HTMLElement; from: number; to: number }>
  svg: SVGSVGElement
}

let mounted = 0

export const mountGantt: MountPane = (root, ctx): PaneView => {
  const { document: doc, copy, intents } = ctx
  const words = copy.gantt
  const win = doc.defaultView
  const SVG = 'http://www.w3.org/2000/svg'
  const uid = `moai-gantt-${++mounted}`

  let zoom: Zoom = DEFAULT_ZOOM
  let gridW = loadGrid()
  let state: State | null = null
  let model: Model | null = null
  let axis: Axis | null = null
  let built: Built | null = null
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
  /** Rows whose planned days are being written: a bar drawn from the read before that write does not move again. */
  const writing = new Set<string>()

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
    const node = el(doc, 'div', {
      class: `g-row${g?.deferred ? ' dim' : ''}`, role: 'option', 'aria-selected': String(line.key === cursor), 'data-id': row ? row.id : null, 'data-key': line.key,
    })
    const title = el(doc, 'span', { class: `c-title${line.dir ? ' grp' : ''}${row?.kind === 'milestone' ? ' ms' : ''}` })
    const twig = twigOf(line)
    if (twig) title.append(twig)
    if (row) {
      const { glyph, tone } = statusGlyph(statusOf(row), m.boot)
      const name = String(row.title ?? row.id)
      title.append(row.kind === 'milestone' ? '◆ ' : '', name)
      title.title = name
      const progress = g?.progress.kind === 'rollup' && g.progress.total > 0 ? `${g.progress.done}/${g.progress.total}` : ''
      const st = el(doc, 'span', { class: `c-st m-st-${tone}`, title: (copy.stats as Record<string, string>)[statusOf(row)] ?? statusOf(row) }, glyph)
      node.append(el(doc, 'span', { class: 'c-id' }, row.id), st, title, el(doc, 'span', { class: 'c-n' }, progress), el(doc, 'span', { class: 'c-who' }, assigneeOf(row)))
    } else {
      title.append(line.kind === 'bucket' ? segLabel(line.seg, m.index, copy) : '')
      node.append(el(doc, 'span', { class: 'c-id' }), el(doc, 'span', { class: 'c-st' }), title, el(doc, 'span', { class: 'c-n' }), el(doc, 'span', { class: 'c-who' }))
    }
    node.addEventListener('click', () => (row ? intents.choose(line.key) : intents.point(line.key)))
    return node
  }

  function draw(): void {
    if (!model || !state) return
    if (holding) return void (drawLater = true)
    drawLater = false
    const m = model
    const now = nowWall()
    const zone = clock.zone()
    const placed = m.lines.map((line) => (line.kind === 'row' ? placeRow(m.index.rows[line.at]!.gantt, m.index.rows[line.at], now, zone) : null))
    const was = axis
    if (!axis || axis.zoom !== zoom || !fresh(placed, now)) {
      const [from, to] = rangeOf(placed.map((p) => p?.span ?? null), now)
      axis = { zoom, from, to }
    }
    const a = axis
    // The bars this hovered bubble spoke of are about to be replaced, and a removed node sends no mouseleave.
    bubble.hidden = true
    const cursor = state.cursor
    const rows: HTMLElement[] = []
    const lanes: HTMLElement[] = []
    const clips: Built['clips'] = []

    const wide = widthOf(a)
    right.style.width = `${wide}px`
    lanesBox.style.height = `${m.lines.length * ROW_H}px`
    // An axis that grew at its start (a new row reaches earlier) moves every date right: the same dates stay in view.
    // A new step is setZoom's and fit's to place.
    if (was && was.zoom === a.zoom && was.from !== a.from) scroll.scrollLeft = Math.max(0, scroll.scrollLeft + xOf(a, was.from))

    // Weekends, behind everything. Not at the month step, where a day is 4px.
    const shade: HTMLElement[] = []
    if (a.zoom !== 'month') {
      for (const [from, to] of weekends(a)) {
        const col = el(doc, 'div', { class: 'col-we' })
        col.style.left = `${xOf(a, from)}px`
        col.style.width = `${xOf(a, to) - xOf(a, from)}px`
        shade.push(col)
      }
    }
    const svg = doc.createElementNS(SVG, 'svg')
    svg.setAttribute('class', 'links')
    svg.setAttribute('width', String(wide))
    svg.setAttribute('height', String(m.lines.length * ROW_H))
    svg.setAttribute('aria-hidden', 'true')

    m.lines.forEach((line, n) => {
      const g = line.kind === 'row' ? m.index.rows[line.at]!.gantt : undefined
      const p = placed[n] ?? null
      rows.push(gridRow(m, line, g, cursor))
      const lane = el(doc, 'div', { class: `lane${g?.deferred ? ' dim' : ''}`, 'data-id': line.kind === 'row' ? m.index.rows[line.at]!.id : null, 'data-key': line.key, 'aria-selected': String(line.key === cursor) })
      lane.addEventListener('click', () => {
        // The click that ends a bar's drag is the drag's, not a choice.
        if (dragged) return void (dragged = false)
        if (line.kind === 'row') intents.choose(line.key)
        else intents.point(line.key)
      })
      if (line.kind === 'row') plannable(lane, m.index.rows[line.at]!, g)
      if (p && g) {
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
        const l = mark(lane, 'clip', '‹', 0)
        const r = mark(lane, 'clip', '›', 0)
        l.hidden = r.hidden = true
        clips.push(p.span ? { left: l, right: r } : null)
      } else clips.push(null)
      lanes.push(lane)
    })
    const today = el(doc, 'div', { class: 'todayline' })
    today.style.left = `${xOf(a, now)}px`
    lanesBox.replaceChildren(...shade, ...lanes, svg, today)
    rowsBox.replaceChildren(...rows)
    rowsBox.style.height = `${m.lines.length * ROW_H}px`

    // The header; its today cap stands over the day's line.
    const top: Built['top'] = []
    const header = headerOf(a, now, words)
    const ticks = (box: HTMLElement, list: Tick[], keep: boolean) => {
      box.replaceChildren(...list.map((tick) => {
        const node = el(doc, 'div', { class: `tick${tick.cls ? ` ${tick.cls}` : ''}` }, tick.text)
        node.style.left = `${xOf(a, tick.from)}px`
        node.style.width = `${xOf(a, tick.to) - xOf(a, tick.from)}px`
        if (keep) top.push({ el: node, from: xOf(a, tick.from), to: xOf(a, tick.to) })
        return node
      }))
    }
    ticks(hd1, header.top, true)
    ticks(hd2, header.bottom, false)
    const cap = el(doc, 'div', { class: 'todaycap', 'aria-hidden': 'true' })
    cap.style.left = `${xOf(a, now)}px`
    hd1.append(cap)

    built = { rows, lanes, clips, placed, top, svg }
    empty.hidden = m.lines.length > 0
    empty.replaceChildren(...(m.lines.length ? [] : [emptyState(doc, { icon: 'gantt-chart', title: words.empty })]))
    for (const [z, b] of zoomButtons) b.setAttribute('aria-pressed', String(z === zoom))
    paintArrows()
    paintView()
  }

  /** The axis stands while the lines' dates still fit in it; a draw because the index was read again does not shift it. */
  function fresh(placed: Array<Placed | null>, now: number): boolean {
    if (!axis) return false
    const [from, to] = rangeOf(placed.map((p) => p?.span ?? null), now)
    return from >= axis.from && to <= axis.to
  }

  /** The arrows into the cursor row, from what blocks it: one SVG behind the bars, the cursor row's only. */
  function paintArrows(): void {
    if (!built || !model || !axis || !state) return
    const svg = built.svg
    svg.replaceChildren()
    const to = model.lines.findIndex((line) => line.key === state!.cursor)
    const line = model.lines[to]
    const target = to < 0 ? null : built.placed[to]
    if (!line || line.kind !== 'row' || !target) return
    const g = model.index.rows[line.at]!.gantt
    const x1 = target.plan?.from ?? target.span?.[0]
    if (!g || x1 === undefined) return
    for (const block of g.blocked_by) {
      const from = model.lines.findIndex((one) => one.key === block.id)
      const source = from < 0 ? null : built.placed[from]
      const x0 = source ? source.plan?.to ?? source.span?.[1] : undefined
      if (!source || x0 === undefined) continue
      const { d, head } = arrowOf(xOf(axis, x0), from, xOf(axis, x1), to)
      const path = doc.createElementNS(SVG, 'path')
      path.setAttribute('d', d)
      const tipHead = doc.createElementNS(SVG, 'polygon')
      tipHead.setAttribute('points', head)
      svg.append(path, tipHead)
    }
  }

  /** What depends on the scroll offset: the `‹ ›` marks, the header labels kept in view, the range. */
  function paintView(): void {
    if (!built || !axis) return
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
    model.lines.forEach((line, n) => {
      const on = String(line.key === cursor)
      for (const node of [built!.rows[n]!, built!.lanes[n]!]) if (node.getAttribute('aria-selected') !== on) node.setAttribute('aria-selected', on)
    })
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
      if (g.planned) add(g.planned.from === 'members' ? t.plannedMembers : t.planned, `${g.planned.start ?? '—'} ~ ${g.planned.end ?? '—'}`)
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
    draw()
    scroll.scrollLeft = Math.max(0, xOf(axis, centre) - held)
    paintView()
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
      draw()
    }
    scroll.scrollLeft = Math.max(0, xOf(axis, span[0]) - 8)
    paintView()
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

  function command(action: string): boolean {
    if (!model || !state) return false
    const page = Math.max(1, Math.floor((scroll.clientHeight || 720) / ROW_H) - 2)
    switch (action) {
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
        const half = Math.max(ROW_H, Math.floor((scroll.clientHeight || 720) / 2))
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
    if (!waiting && !drawLater) return
    setTimeout(() => {
      // Another drag took the lanes meanwhile: its release draws.
      if (holding) return
      const next = waiting
      waiting = null
      if (next) render(...next)
      if (drawLater) draw()
    }, 0)
  }

  /**
   * Write a row's planned days. One write at a time per row: until it is answered (and the rows read again), the
   * bar on screen was drawn from the read before it, and a move measured from there would undo the first.
   */
  function write(movePlan: NonNullable<typeof ctx.movePlan>, id: string, change: PlanChange, undo: string): void {
    writing.add(id)
    // The bar stays where it was dropped until the rows are read again; a refusal draws it back.
    const drawn = model
    void movePlan(id, change, undo).finally(() => {
      writing.delete(id)
      if (model === drawn) draw()
    })
  }

  /** A press on the row's own planned bar, or on its `◆`, drags it. The lane takes the press: the actual band lies over the bar. */
  function movable(lane: HTMLElement, row: Row, g: GanttRow, planBar: HTMLElement | null, dueMark: HTMLElement | null): void {
    const movePlan = ctx.movePlan
    const grab = planBar ?? dueMark
    if (!movePlan || !grab || !axis || overlaid(row)) return
    // A bar or `◆` drawn from a day of the row's own that the plan carries no day for (a milestone's deadline the plan
    // did not read) has nothing to move: it is not grabbed, rather than refused on release.
    const days = g.planned && (dayStart(g.planned.start) !== null || dayStart(g.planned.end) !== null)
    if (!days) return
    const a = axis
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
      const span = 'why' in change ? null : movedPlan(g.planned, change)
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
        // The days as the list printed them; a row it did not print (one written between its read and the tree's) as
        // the bar was drawn — the days this move was measured from.
        const planned = g.planned?.from === 'own' ? g.planned : null
        write(movePlan, row.id, { edge, days }, undoLine(row.id, text(row.starts_on) ?? planned?.start, text(row.due_on) ?? planned?.end))
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
    if (!movePlan || !axis || written || overlaid(row) || row.kind === 'epic' || row.kind === 'milestone' || g?.planned) return
    const a = axis
    lane.addEventListener('dblclick', (event) => {
      // A second double click before the first is answered would plan the row twice.
      if (writing.has(row.id)) return
      const day = dayText(floorDay(wallAt(a, event.clientX - lane.getBoundingClientRect().left)))
      write(movePlan, row.id, { day }, undoLine(row.id, null, null))
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
    const key = JSON.stringify([current.path, current.view, current.search.text, current.filter.applied])
    const zone = clock.zone()
    if (!model || model.index !== index || model.key !== key || model.matched !== current.matched || model.boot !== current.boot || model.zone !== zone) {
      const found = narrow(current, index)
      model = { index, key, matched: current.matched, boot: current.boot, zone, lines: ganttLines(index, current.path, found.keep) }
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
