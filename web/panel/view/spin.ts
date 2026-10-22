/**
 * What someone holds moves, as it does in the TUI (se-gowr.oie): its status glyph turns through the braille spinner
 * and a yellow band runs over its title. Which rows move is the TUI's rule (`tui::App::spins`): work in a started
 * column (in progress, review), and a group whose work is, unless deferral took it out of the plan. The motion itself
 * is components.css; this file marks what moves and times the band.
 */
import { isGroup, isWork, NO_MILESTONE, type IssueIndex } from '../data/index.js'
import { el } from './components.ts'
import { statusOf } from './list-model.ts'

/** The TUI's step: one spinner frame, and one cell of the band, every 120 ms (`SPIN_TICK`). */
const STEP_MS = 120
/** The band's cells, and the cells it rests past the end before it comes in again (`glint()`, `GLINT_REST`). */
const BAND = 3
const REST = 8

/**
 * Does the row at `at` move? The TUI's rule (`tui::App::spins`). Work moves in a started column (`Config::is_started`):
 * one that is neither the first nor done — in progress, review, and a column of the project's own between them — and
 * one the config knows (`cfg.knows`: a column left over from an older config, or written by hand, stands still). A
 * group moves only while it is busy (`report::Stand::busy`): some work under it moves. Its own column says less — a
 * group stands in a started column as soon as one member is done and the rest wait (`column_of`), which is half done,
 * not being done. Deferred work stands still, and so does work a deferred group took out of the plan. `first` is the
 * project's first configured column and `known` its columns (`/api/boot`'s `statuses`); until the boot answers, moai's
 * default first column and no check of the rest.
 */
export function spins(index: IssueIndex, at: number, first: string = FIRST, known?: readonly string[]): boolean {
  if (index.shelvedAt(at) !== undefined) return false
  const row = index.rows[at]!
  return isGroup(row) ? busyGroups(index, first, known).has(at) : started(statusOf(row), first, known)
}

const started = (status: string, first: string, known: readonly string[] | undefined): boolean =>
  status !== '' && status !== first && status !== 'done' && (known === undefined || known.includes(status))

/** The groups some moving work stands under, worked out once per index and rule: work that moves moves every group above it. */
const busyMemo = new WeakMap<IssueIndex, { rule: string; groups: ReadonlySet<number> }>()

function busyGroups(index: IssueIndex, first: string, known: readonly string[] | undefined): ReadonlySet<number> {
  const rule = JSON.stringify([first, known ?? null])
  const kept = busyMemo.get(index)
  if (kept?.rule === rule) return kept.groups
  const groups = new Set<number>()
  index.rows.forEach((row, at) => {
    if (!isWork(row) || index.shelvedAt(at) !== undefined || !started(statusOf(row), first, known)) return
    for (const seg of index.homeOf(at)) {
      // `m:<id>` and `e:<id>` name the groups it stands in; the `(no milestone)` bucket and an issue's own folder do not.
      if (!((seg.startsWith('m:') && seg !== NO_MILESTONE) || seg.startsWith('e:'))) continue
      const group = index.find(seg.slice(2))
      if (group !== undefined) groups.add(group)
    }
  })
  busyMemo.set(index, { rule, groups })
  return groups
}

/** The first column, where nothing has started (`Config::first_status`), when the configured ones are not known yet. */
export const FIRST = 'todo'

/** The first configured column, or moai's default before `/api/boot` answers. */
export const firstStatus = (boot: { statuses: readonly string[] } | null | undefined): string => boot?.statuses[0] ?? FIRST

/** Wide letters (Hangul, CJK, full-width forms) take two cells, as in the terminal. */
const WIDE = /[ᄀ-ᅟ⺀-〾ぁ-㏿㐀-䶿一-鿿ꥠ-꥿가-힣豈-﫿︰-﹏＀-｠￠-￦]/u

export function cells(text: string): number {
  let n = 0
  for (const c of text) n += WIDE.test(c) ? 2 : 1
  return n
}

/** One trip of the band over `text`: in from the left, past the end, then the rest — a cell a step. */
export const glintTime = (text: string): number => (cells(text) + BAND + REST) * STEP_MS

/** The title's words in the band; the caller keeps chips and marks outside it, so they keep their own colour. */
export function glint(doc: Document, text: string, ...content: Array<Node | string>): HTMLElement {
  const band = el(doc, 'span', { class: 'm-glint' }, ...(content.length ? content : [text]))
  // A custom property through CSSOM: the CSP's style-src lets it through where a `style` attribute it would not.
  band.style.setProperty('--glint-time', `${glintTime(text)}ms`)
  return band
}
