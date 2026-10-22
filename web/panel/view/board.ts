/**
 * The kanban: status columns (with Backlog and deferred in front), milestone lanes at the root, cards of up
 * to three lines. Which card goes where is board-groups.ts (on where moai stands each row); this file draws it
 * with the shared components. It reads state and reports a click as `intents.choose`; moving a card
 * between columns is not part of 2.0. Keys are the key router's (panel/keys/): it calls command() — j/k
 * within a column, h/l to the next column, Enter to show the card — and the cards listen for none.
 */
import { isDone, type IssueIndex, type Row } from '../data/index.js'
import { assigneeOf } from '../data/people.js'
import { isUnread } from '../data/read.js'
import type { Boot } from '../data/source.js'
import type { State } from '../store.js'
import { BACKLOG, DEFERRED, groupBoard, type BoardLane, type BoardModel } from './board-groups.ts'
import { segLabel } from './crumbs.ts'
import { narrow, segKey, statusGlyph, statusOf } from './list-model.ts'
import { chip, el, emptyState } from './components.ts'
import { newChip } from './unread.ts'
import { firstStatus, glint, spins } from './spin.ts'
import { branchMark, branchOf } from './branch.ts'
import type { MountPane, PaneView, ViewContext } from './types.ts'

/**
 * A column head's glyph, as moai draws it (`/api/boot`: `columns` for the backlog and deferred columns, `glyphs`
 * for the statuses), so a column is never told by colour alone. Before the boot answers, the explorer's own.
 */
export const glyphOf = (key: string, boot?: Boot | null): string =>
  key === BACKLOG ? boot?.columns.backlog.glyph ?? '◇' : key === DEFERRED ? boot?.columns.shelved.glyph ?? '‖' : statusGlyph(key, boot).glyph

/** components.css 5.7: a column's minimum width, the gap between two columns, the board's padding. */
const LANE_MIN = 200
const LANE_GAP = 4
const BOARD_PAD = 6

const priorityClass = (priority: number) => `m-p${Math.min(3, Math.max(0, Math.round(priority)))}`

function draw(ctx: ViewContext, state: State, index: IssueIndex, model: BoardModel): HTMLElement[] {
  const { document: doc, copy, intents } = ctx
  const firstColumn = firstStatus(state.boot)
  if (!model.total) return [emptyState(doc, { icon: 'kanban', title: copy.empty })]

  const nameOf = (key: string) =>
    key === BACKLOG ? state.boot?.columns.backlog.name ?? copy.shell.backlog
      : key === DEFERRED ? state.boot?.columns.shelved.name ?? copy.shell.deferred
      : ((copy.stats as Record<string, string>)[key] ?? key)

  // One card is the tab stop, where focus() puts the focus: the one with the cursor, else the first. Tab on the board
  // runs nothing (expandAll is the list's), so the key router leaves it to the browser: Tab reaches that one card and
  // goes on past the board, and j/k/h/l move between the cards.
  const cards = model.lanes.flatMap((lane) => lane.columns.flatMap((column) => column.cards))
  const cursorCard = state.cursor === null ? undefined : cards.find((at) => index.rows[at]!.id === state.cursor)
  const tabStop = cursorCard ?? cards[0]

  const card = (at: number): HTMLElement => {
    const row: Row = index.rows[at]!
    const key = row.id
    const done = isDone(row)
    const selected = state.cursor === key
    const first = el(doc, 'div', { class: 'm-card-line' })
    first.append(el(doc, 'span', { class: 'm-id' }, row.id))
    // The status in the card itself, as the TUI draws it (`id glyph pN n/n`): a card in the Deferred or Backlog
    // column would otherwise not say where it stands. The glyph differs by status, never colour alone.
    const status = statusOf(row)
    const { glyph, tone } = statusGlyph(status, state.boot)
    // What someone holds moves as in the TUI (view/spin.ts): the glyph turns and a band runs over the title.
    const moving = spins(index, at, firstColumn)
    first.append(' ', el(doc, 'span', { class: `m-st-${tone}${moving ? ' m-spin' : ''}`, title: (copy.stats as Record<string, string>)[status] ?? status }, glyph))
    if (typeof row.priority === 'number') first.append(' ', el(doc, 'span', { class: priorityClass(row.priority) }, `p${row.priority}`))
    if (index.isDir(at)) {
      const [d, w] = index.tally(index.dirPath(at))
      if (w) first.append(' ', el(doc, 'span', { class: 'm-count' }, `${d}/${w}`))
    }
    // Another worktree holds it: `⎇ <branch>`, as the TUI marks the row (view/branch.ts, se-etwu).
    const branch = branchOf(row)
    if (branch) first.append(' ', branchMark(doc, branch))
    const title = el(doc, 'div', { class: 'm-card-title' })
    if (isUnread(row)) title.append(newChip(doc), ' ')
    const name = row.title ?? row.id
    title.append(moving ? glint(doc, name) : name)
    const lines: Node[] = [first, title]
    const extras: HTMLElement[] = []
    const who = assigneeOf(row)
    if (state.columns.assignee && who) extras.push(chip(doc, { label: who }))
    if (state.columns.epic) {
      const epic = index.epicOf(at)
      if (epic !== undefined) extras.push(chip(doc, { label: index.rows[index.find(epic) ?? -1]?.title ?? epic, tone: 'accent' }))
    }
    if (state.columns.tags && Array.isArray(row.tags)) for (const tag of row.tags) extras.push(chip(doc, { label: `#${tag}` }))
    // A chip is cut at the card's edge (board.css, moai-rqec); its whole words stay in its title.
    for (const extra of extras) extra.title = extra.textContent ?? ''
    if (extras.length) lines.push(el(doc, 'div', { class: 'm-card-line' }, ...extras))
    const node = el(doc, 'div', {
      class: 'm-card', role: 'listitem', tabindex: at === tabStop ? 0 : -1, 'aria-selected': String(selected),
      'data-id': row.id, 'data-focus': `card:${row.id}`, title: row.title ?? row.id,
    }, ...lines)
    if (done) node.style.opacity = '0.75'
    node.addEventListener('click', () => intents.choose(row.id))
    return node
  }

  const laneHead = (lane: BoardLane): HTMLElement | null => {
    if (lane.seg === null) return null
    const title = lane.at !== undefined ? (index.rows[lane.at]!.title ?? index.rows[lane.at]!.id) : segLabel(lane.seg, index, copy)
    const head = el(doc, 'div', { class: 'm-swim-head', 'data-lane': lane.seg })
    const count = lane.columns.reduce((n, column) => n + column.cards.length, 0)
    head.append(el(doc, 'span', {}, title), el(doc, 'span', { class: 'm-count' }, String(count)))
    return head
  }

  const out: HTMLElement[] = []
  for (const lane of model.lanes) {
    const head = laneHead(lane)
    if (head) out.push(head)
    const board = el(doc, 'div', { class: 'm-board', 'data-lane': lane.seg ?? '' })
    for (const column of lane.columns) {
      const label = nameOf(column.key)
      const lanes = el(doc, 'div', { class: 'm-lane', role: 'list', 'data-status': column.key, 'aria-label': copy.board.columnLabel(label, column.cards.length) })
      // The column's glyph turns while a card in it does, as the TUI's column head does.
      const turning = column.cards.some((at) => spins(index, at, firstColumn))
      lanes.append(el(doc, 'div', { class: 'm-lane-head' },
        el(doc, 'span', { 'aria-hidden': 'true', class: turning ? `m-spin m-st-${column.key}` : null }, glyphOf(column.key, state.boot)), el(doc, 'span', {}, label), el(doc, 'span', { class: 'm-count' }, String(column.cards.length))))
      if (!column.cards.length) lanes.append(el(doc, 'div', { class: 'm-lane-empty' }, copy.board.noCards))
      for (const at of column.cards) lanes.append(card(at))
      board.append(lanes)
    }
    out.push(board)
  }
  // Narrow panels scroll sideways instead of squeezing the columns below their minimum. The numbers are the
  // stylesheet's (components.css 5.7): a column's minimum, the gap between columns, the board's padding.
  const columns = model.keys.length
  const wrap = el(doc, 'div', { class: 'm-board-wrap', style: `min-width: ${columns * LANE_MIN + (columns - 1) * LANE_GAP + 2 * BOARD_PAD}px;` })
  wrap.append(...out)
  return [wrap]
}

/** Where the cursor card stands: its lane, column and place in the column; null when the cursor is not a card. */
function locate(model: BoardModel, index: IssueIndex, cursor: string | null): { lane: number; column: number; at: number } | null {
  if (cursor === null) return null
  for (let lane = 0; lane < model.lanes.length; lane++) {
    const columns = model.lanes[lane]!.columns
    for (let column = 0; column < columns.length; column++) {
      const at = columns[column]!.cards.findIndex((card) => index.rows[card]!.id === cursor)
      if (at >= 0) return { lane, column, at }
    }
  }
  return null
}

export const mountBoard: MountPane = (root, ctx): PaneView => {
  const doc = ctx.document
  const pane = el(doc, 'div', { class: 'm-board-pane', 'aria-label': ctx.copy.board.label })
  root.replaceChildren(pane)
  let cursor: string | null = null
  let drawn: { state: State; index: IssueIndex; model: BoardModel; key: string } | null = null

  /**
   * Nothing but the cursor moved (the cards, their order and the columns are what they were): move the selection mark
   * and the tab stop between cards instead of drawing the board again. A change that is not the board's at all (the
   * detail loading, the focus, a report) leaves the DOM as it is, whatever the cursor stands on. False when that cannot
   * be told from the DOM: a new cursor with no card drawn (it may add a column), or a card or a hidden column drawn
   * only because the cursor was there (BoardModel.pinned). The archive age is judged at the last full draw, as
   * narrow() memoizes it, until the next read.
   */
  function inPlace(state: State, index: IssueIndex, drawKey: string): boolean {
    // What moai found is drawn from too: its answer comes after the filter or search that asked it, with the rest of
    // the key unchanged (the list's model keys on it the same way).
    if (!drawn || drawn.index !== index || drawn.key !== drawKey || drawn.state.matched !== state.matched) return false
    if (drawn.state.cursor === state.cursor) {
      drawn.state = state
      return true
    }
    if (drawn.model.pinned || !drawn.model.total) return false
    const cards = [...pane.querySelectorAll<HTMLElement>('.m-card')]
    const target = state.cursor === null ? undefined : cards.find((node) => node.dataset.id === state.cursor)
    if (state.cursor !== null && !target) return false
    for (const node of cards) {
      const selected = String(node === target)
      if (node.getAttribute('aria-selected') !== selected) node.setAttribute('aria-selected', selected)
    }
    const stop = target ?? cards[0]
    for (const node of cards) {
      const want = node === stop ? '0' : '-1'
      if (node.getAttribute('tabindex') !== want) node.setAttribute('tabindex', want)
    }
    drawn.state = state
    return true
  }

  /**
   * A key-table action (keys/wiring.ts): moves stay inside the cursor's column — one run from the top lane to the
   * bottom one, as the TUI's board walks it — and h/l go to the next column that has cards in the cursor's lane.
   */
  function command(action: string): boolean {
    if (!drawn) return false
    const { state, index, model } = drawn
    const point = (card: number | undefined) => {
      if (card !== undefined) ctx.intents.point(index.rows[card]!.id)
    }
    const first = model.lanes.flatMap((lane) => lane.columns.flatMap((column) => column.cards))[0]
    const here = locate(model, index, state.cursor)
    const page = Math.max(1, Math.floor((root.clientHeight || 720) / 80))
    switch (action) {
      case 'move.down':
      case 'move.up':
      case 'move.pageDown':
      case 'move.pageUp':
      case 'move.top':
      case 'move.bottom': {
        if (!here) {
          point(first)
          return true
        }
        // Every lane has the same columns: the column is one run across the lanes (milestones at the root), so j/k and
        // gg/G reach the cards of the other lanes too.
        const run = model.lanes.flatMap((lane) => lane.columns[here.column]!.cards)
        const at = run.indexOf(model.lanes[here.lane]!.columns[here.column]!.cards[here.at]!)
        const step = { 'move.down': 1, 'move.up': -1, 'move.pageDown': page, 'move.pageUp': -page, 'move.top': -Infinity, 'move.bottom': Infinity }[action]
        point(run[Math.max(0, Math.min(run.length - 1, at + step))])
        return true
      }
      case 'move.halfDown':
      case 'move.halfUp': {
        const half = Math.max(40, Math.floor((root.clientHeight || 720) / 2))
        root.scrollTop = Math.max(0, root.scrollTop + (action === 'move.halfDown' ? half : -half))
        return true
      }
      case 'expand':
      case 'collapse': {
        if (!here) {
          point(first)
          return true
        }
        const columns = model.lanes[here.lane]!.columns
        const delta = action === 'expand' ? 1 : -1
        for (let next = here.column + delta; next >= 0 && next < columns.length; next += delta) {
          const cards = columns[next]!.cards
          if (cards.length) {
            point(cards[Math.min(here.at, cards.length - 1)])
            break
          }
        }
        return true
      }
      case 'enter': {
        // The cursor's card; with the cursor on no card (a group or `..` in the list), the card that has the focus —
        // the tab stop focus() moved to — as the card's own Enter chose it before the key router took the keys.
        const focused = doc.activeElement?.closest<HTMLElement>('.m-card')
        const id = here ? state.cursor! : focused && pane.contains(focused) ? focused.dataset.id : undefined
        if (id === undefined) return true
        ctx.intents.choose(id)
        if (ctx.layout().detailVisible) ctx.focusPane('detail')
        return true
      }
      case 'leave': {
        const path = state.path
        if (path.length) ctx.intents.enter(path.slice(0, -1), segKey(path[path.length - 1]!))
        return true
      }
    }
    return false
  }

  return {
    render(state, index) {
      const active = doc.activeElement as HTMLElement | null
      const key = active && pane.contains(active) ? active.dataset?.focus : undefined
      if (index) {
        // What the cards are drawn from. The cursor is not in it: a cursor move changes two cards' marks, not the board.
        const drawKey = JSON.stringify([state.path, state.view, state.search.text, state.filter.applied, state.columns])
        if (!inPlace(state, index, drawKey)) {
          // The board is a layout of the list: the same filter and search narrow it (list-model.ts narrow).
          const found = narrow(state, index)
          const model = groupBoard(index, { path: state.path, view: state.view, cursor: state.cursor, match: found.match, reveal: found.searching }, state.boot?.statuses)
          drawn = { state, index, model, key: drawKey }
          pane.replaceChildren(...draw(ctx, state, index, model))
        }
      } else {
        drawn = null
        pane.replaceChildren()
      }
      if (key !== undefined) {
        // A focused card follows the cursor a key moved, so the focus ring and the selection stay one card.
        const want = key.startsWith('card:') && state.cursor !== null ? [`card:${state.cursor}`, key] : [key]
        const nodes = [...pane.querySelectorAll<HTMLElement>('[data-focus]')]
        const node = want.map((one) => nodes.find((n) => n.dataset.focus === one)).find((n) => n !== undefined)
        node?.focus({ preventScroll: true })
      }
      // The card with the cursor stays in view when the cursor moves.
      if (state.cursor !== cursor) {
        cursor = state.cursor
        const selected = pane.querySelector<HTMLElement>('.m-card[aria-selected="true"]')
        if (selected && typeof selected.scrollIntoView === 'function') selected.scrollIntoView({ block: 'nearest', inline: 'nearest' })
      }
    },
    command,
    focus() {
      const target = pane.querySelector<HTMLElement>('.m-card[aria-selected="true"]') ?? pane.querySelector<HTMLElement>('.m-card')
      ;(target ?? (pane.closest('[tabindex]') as HTMLElement | null))?.focus({ preventScroll: false })
    },
    destroy() {
      pane.remove()
    },
  }
}
