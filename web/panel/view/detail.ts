/**
 * The detail pane (design §4.3): the row under the cursor from `show <id>` — head line, labeled fields,
 * blockers, a group's roll-up, the body, commits, and the history (journal) and work log as folded sections.
 * It only draws `state.shown` and the index; closing is the one thing it reports (`intents.closeDetail`).
 * The pure wording lives in view/detail-format.ts.
 */
import { isInformation } from '../data/client.js'
import type { IssueIndex, Row } from '../data/index.js'
import { isUnread } from '../data/read.js'
import type { BoardData, State } from '../store.js'
import { banner, button, chip, el, emptyState } from './components.ts'
import {
  BLOCKER_MARK,
  blockedByOf,
  blockerState,
  blocksOf,
  childrenOf,
  commitsOf,
  fieldsOf,
  headOf,
  journalOf,
  problemText,
  rollupOf,
  tagsOf,
  workOf,
} from './detail-format.ts'
import { renderMarkdown } from './markdown.ts'
import { newChip } from './unread.ts'
import { highlight } from './toolbar.ts'
import type { MountPane, PaneView, ViewContext } from './types.ts'

/** The milestone and epic counts per column the roll-up reads (main.ts reads them beside the rows). */
type Groups = BoardData['groups']

/** The body as markdown (view/markdown.ts: raw HTML stays text, links are only recorded, never followed). */
export function renderBody(doc: Document, text: string): HTMLElement {
  const { root } = renderMarkdown(doc, text)
  root.classList.add('moai-detail-body')
  return root
}

const STYLE_ID = 'moai-detail-style'
const STYLE = `
.moai-detail { display: flex; flex-direction: column; gap: 10px; padding: 10px 12px 14px; font-size: 13px; }
.moai-detail-head { display: flex; align-items: flex-start; gap: 8px; }
.moai-detail-headtext { flex: 1; min-width: 0; }
.moai-detail-id { color: var(--muted); font-variant-numeric: tabular-nums; font-size: 12px; }
.moai-detail-title { margin: 2px 0 6px; font-size: 15px; font-weight: 600; overflow-wrap: anywhere; }
.moai-detail-chips { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; }
.moai-detail-fields { display: grid; grid-template-columns: max-content minmax(0, 1fr); gap: 4px 12px; margin: 0; }
.moai-detail-fields dt { color: var(--muted); }
.moai-detail-fields dd { margin: 0; overflow-wrap: anywhere; }
.moai-detail-ids { margin: 0; padding: 0; list-style: none; }
.moai-detail-ids li { overflow-wrap: anywhere; }
.moai-detail-mark { display: inline-block; width: 1.2em; color: var(--muted); }
.moai-detail-bar { height: 6px; border-radius: 3px; background: var(--border); overflow: hidden; }
.moai-detail-bar > span { display: block; height: 100%; background: var(--ok); }
.moai-detail-body { overflow-wrap: anywhere; }
.moai-detail-raw { margin: 0; white-space: pre-wrap; font-family: var(--mono, monospace); font-size: 12px; }
.moai-detail h3, .moai-detail summary { margin: 0; font-size: 12px; font-weight: 600; color: var(--muted); }
.moai-detail summary { cursor: pointer; }
.moai-detail-list { margin: 4px 0 0; padding: 0; list-style: none; }
.moai-detail-list li { display: flex; flex-wrap: wrap; gap: 2px 10px; padding: 2px 0; overflow-wrap: anywhere; }
.moai-detail-hash { font-family: var(--mono, monospace); color: var(--muted); }
.moai-detail-meta { color: var(--muted); }
`

function styled(doc: Document) {
  if (doc.getElementById(STYLE_ID)) return
  const style = el(doc, 'style', { id: STYLE_ID })
  style.textContent = STYLE
  doc.head.append(style)
}

/** A titled block; `h3` is the section's name. */
const section = (doc: Document, title: string, ...content: Array<Node | null>) => el(doc, 'section', {}, el(doc, 'h3', {}, title), ...content)

/** A folded block: the summary is its name, the open state survives redraws through `open`. */
function folded(doc: Document, key: string, title: string, open: Set<string>, ...content: Node[]) {
  const details = el(doc, 'details', { 'data-fold': key, open: open.has(key) }, el(doc, 'summary', {}, title), ...content)
  details.addEventListener('toggle', () => {
    if (details.open) open.add(key)
    else open.delete(key)
  })
  return details
}

function idList(doc: Document, ids: string[], index: IssueIndex | null, copy: ViewContext['copy'], marks: boolean, row?: Row): HTMLElement {
  const list = el(doc, 'ul', { class: 'moai-detail-ids' })
  for (const id of ids) {
    const item = el(doc, 'li')
    const at = index?.find(id)
    const title = at === undefined ? '' : String(index!.rows[at]!.title ?? '')
    if (marks) {
      const state = blockerState(id, index, row)
      const word = { done: copy.detail.blockerDone, open: copy.detail.blockerOpen, deferred: copy.detail.blockerDeferred, missing: copy.detail.blockerMissing }[state]
      item.append(el(doc, 'span', { class: 'moai-detail-mark', title: word, 'aria-label': word }, BLOCKER_MARK[state]))
    }
    item.append(`${id}${title ? ` ${title}` : ''}`)
    list.append(item)
  }
  return list
}

export const mountDetail: MountPane = (root: HTMLElement, ctx: ViewContext): PaneView => {
  const { document: doc, copy, intents } = ctx
  const d = copy.detail
  styled(doc)
  const view = el(doc, 'div', { class: 'moai-detail', role: 'region', 'aria-label': d.label })
  root.replaceChildren(view)
  // The pane scrolls in the split's detail container (marked data-keep-scroll), which holds `root`; a view
  // mounted on a scroller of its own is its own.
  const scroller = (): HTMLElement => root.closest<HTMLElement>('[data-keep-scroll]') ?? root
  /** Folded sections the user opened, kept per id so a refresh does not close them. */
  const open = new Set<string>()
  let shownId: string | null = null
  /** What the pane was drawn from: it is drawn again only when one of them changed (a redraw loses a text
   *  selection and the focus on a folded section's summary, and the shell renders it on every store change). */
  let drawn: { shown: State['shown']; index: IssueIndex | null; search: State['search']; groups: Groups } | null = null
  /** The id and title lines of the row drawn, with their text: a search repaints these two and nothing else. */
  let heads: { id: HTMLElement; title: HTMLElement; idText: string; titleText: string } | null = null
  /** `SPC v r`: the body as the markdown it was written in. */
  let raw = false
  let last: { state: State; index: IssueIndex | null } | null = null

  function closeButton() {
    const close = button(doc, { label: d.close, variant: 'ghost', small: true, icon: 'x', onClick: () => intents.closeDetail() })
    close.dataset.focus = 'close'
    return close
  }

  function content(row: Row, index: IssueIndex | null, search: State['search']): Node[] {
    const head = headOf(row, copy)
    const chips = el(doc, 'div', { class: 'moai-detail-chips' })
    // Unread by either copy: `show`'s row, and the row the list holds. The list's row also knows the archive when
    // moai's `show` row does not say it: archived rows are never NEW.
    const listed = index?.find(row.id)
    const held = listed === undefined ? undefined : index!.rows[listed]
    if ((isUnread(row) || (held !== undefined && isUnread(held))) && held?.archived !== true) chips.append(newChip(doc))
    if (head.status) chips.append(chip(doc, { label: head.statusLabel, tone: head.tone }))
    if (head.priority) chips.append(chip(doc, { label: head.priority }))
    if (head.kind) chips.append(chip(doc, { label: head.kind }))
    if (head.deferred) chips.append(chip(doc, { label: `${d.deferred}: ${head.deferred}`, tone: 'warn' }))
    else if (head.shelvedBy) chips.append(chip(doc, { label: d.deferredUnder(head.shelvedBy), tone: 'warn' }))
    for (const tag of tagsOf(row)) chips.append(chip(doc, { label: tag }))
    const idLine = el(doc, 'div', { class: 'moai-detail-id' }, ...painted(head.id, search))
    const titleLine = el(doc, 'h2', { class: 'moai-detail-title' }, ...painted(head.title, search))
    heads = { id: idLine, title: titleLine, idText: head.id, titleText: head.title }
    const out: Node[] = [
      el(doc, 'div', { class: 'moai-detail-head' },
        el(doc, 'div', { class: 'moai-detail-headtext' }, idLine, titleLine, chips),
        closeButton()),
    ]
    const fields = fieldsOf(row, index, copy)
    if (fields.length) {
      const list = el(doc, 'dl', { class: 'moai-detail-fields' })
      for (const field of fields) list.append(el(doc, 'dt', {}, field.label), el(doc, 'dd', { 'data-field': field.key }, field.value))
      out.push(list)
    }
    const blockedBy = blockedByOf(row)
    if (blockedBy.length) out.push(section(doc, d.blockedBy, idList(doc, blockedBy, index, copy, true, row)))
    const blocks = blocksOf(row, index)
    if (blocks.length) out.push(section(doc, d.blocks, idList(doc, blocks, index, copy, false)))
    const rollup = rollupOf(row, index, last?.state.board.value?.groups, last?.state.boot?.statuses)
    if (rollup) {
      const bar = el(doc, 'div', { class: 'moai-detail-bar', role: 'progressbar', 'aria-valuemin': 0, 'aria-valuemax': 100, 'aria-valuenow': rollup.percent ?? 0, 'aria-label': d.rollup })
      bar.append(el(doc, 'span', { style: `width: ${rollup.percent ?? 0}%` }))
      const counts = rollup.byStatus.map(([status, n]) => `${copy.stats[status] ?? status} ${n}`).join(' · ')
      out.push(section(doc, d.rollup, el(doc, 'div', { class: 'moai-detail-rollup' }, d.rollupText(rollup.done, rollup.work, rollup.percent)), bar, counts ? el(doc, 'div', { class: 'moai-detail-meta' }, counts) : null))
    }
    const body = typeof row.body === 'string' ? row.body.trim() : ''
    if (body) out.push(section(doc, d.body, raw ? el(doc, 'pre', { class: 'moai-detail-body moai-detail-raw' }, body) : renderBody(doc, body)))
    const children = childrenOf(row, index)
    if (children.length) {
      const list = el(doc, 'ul', { class: 'moai-detail-list' })
      for (const child of children) list.append(el(doc, 'li', {}, `${child.id} ${child.title ?? ''}`.trim()))
      out.push(section(doc, d.subtasks, list))
    }
    const commits = commitsOf(row)
    if (commits.length) {
      const list = el(doc, 'ul', { class: 'moai-detail-list' })
      for (const commit of commits) list.append(el(doc, 'li', {}, el(doc, 'span', { class: 'moai-detail-hash' }, commit.hash), commit.subject))
      out.push(section(doc, d.commits, list))
    }
    const journal = journalOf(row, copy)
    const history = el(doc, 'ol', { class: 'moai-detail-list' })
    if (journal.length) {
      for (const line of journal) history.append(el(doc, 'li', { 'data-kind': line.kind }, el(doc, 'span', {}, line.text), line.meta ? el(doc, 'span', { class: 'moai-detail-meta' }, line.meta) : null))
    } else history.append(el(doc, 'li', { class: 'moai-detail-meta' }, d.noJournal))
    out.push(folded(doc, 'journal', d.journal, open, history))
    const work = workOf(row)
    if (work.length) {
      const list = el(doc, 'ul', { class: 'moai-detail-list' })
      for (const line of work) list.append(el(doc, 'li', {}, line))
      out.push(folded(doc, 'work', d.work, open, list))
    }
    return out
  }

  /** A search paints its hits in the id and title it looks in (design §4.4); the body is drawn as it is. */
  const painted = (text: string, search: State['search']): Array<Node | string> =>
    search.text.trim() ? highlight(doc, text, search.text) : [text]

  function render(state: State, index: IssueIndex | null) {
    last = { state, index }
    const { value, loading, problem } = state.shown
    // The roll-up's per-column counts are `moai status`'s, which come after the rows (main.ts `groups`): the index stays
    // the same object then, so they key the drawing of their own.
    const groups = state.board.value?.groups
    if (drawn && drawn.shown === state.shown && drawn.index === index && drawn.groups === groups) {
      if (drawn.search === state.search) return
      // Only the search changed (a keystroke): the hits are painted in the id and title, so repaint those two lines.
      // The body, the sections and the markdown stay the nodes they were (and so does a text selection in them).
      const same = drawn.search.text === state.search.text
      drawn = { shown: state.shown, index, search: state.search, groups }
      if (same) return
      if (heads?.id.isConnected && heads.title.isConnected) {
        heads.id.replaceChildren(...painted(heads.idText, state.search))
        heads.title.replaceChildren(...painted(heads.titleText, state.search))
        return
      }
    }
    drawn = { shown: state.shown, index, search: state.search, groups }
    // Another row's detail starts at the top; the same row redrawn (a refresh) keeps its place.
    const id = value?.id ?? null
    const top = id !== null && id === shownId ? scroller().scrollTop : 0
    shownId = id
    const active = doc.activeElement as HTMLElement | null
    const key = active && view.contains(active) ? active.dataset?.focus : undefined
    const children: Node[] = []
    if (problem) {
      const said = problemText(problem, copy)
      children.push(banner(doc, isInformation(problem) ? 'info' : 'danger', said ? `${d.problem} ${said}` : d.problem))
    }
    if (value) children.push(...content(value.row, index, state.search))
    else if (loading) children.push(el(doc, 'p', { class: 'moai-detail-loading', role: 'status' }, copy.loading))
    else if (!problem) children.push(emptyState(doc, { title: d.none }))
    view.replaceChildren(...children)
    scroller().scrollTop = top
    if (key !== undefined) view.querySelector<HTMLElement>(`[data-focus="${key}"]`)?.focus({ preventScroll: true })
  }

  /** A key-table action while the detail has the focus (keys/wiring.ts): the moves scroll it. */
  function command(action: string): boolean {
    const box = scroller()
    const line = 40
    const page = Math.max(line, box.clientHeight - line)
    const by: Record<string, number> = {
      'move.down': line, 'move.up': -line, 'move.halfDown': page / 2, 'move.halfUp': -page / 2,
      'move.pageDown': page, 'move.pageUp': -page, 'move.top': -Infinity, 'move.bottom': Infinity,
    }
    const step = by[action]
    if (step === undefined) return false
    box.scrollTop = step === -Infinity ? 0 : step === Infinity ? box.scrollHeight : Math.max(0, box.scrollTop + step)
    return true
  }

  return {
    render,
    command,
    toggleRaw() {
      raw = !raw
      drawn = null
      if (last) render(last.state, last.index)
    },
    raw: () => raw,
    // The pane itself takes the focus, never its first control: that is Close, and the key router leaves Enter and
    // Space on a focused button to the button (keys/router.ts isActivatable), so SPC meant for the menu, or Enter,
    // would close the detail. From the pane the keys stay the router's (j/k scroll it, SPC opens the menu).
    focus: () => root.focus({ preventScroll: true }),
  }
}
