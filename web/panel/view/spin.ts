/**
 * What someone holds moves, as it does in the TUI (se-gowr.oie): its status glyph turns through the braille spinner
 * and a yellow band runs over its title. Which rows move is the TUI's rule (`tui::App::spins`): work in a started
 * column (in progress, review), and a group whose work is, unless deferral took it out of the plan. The motion itself
 * is components.css; this file marks what moves and times the band.
 */
import type { IssueIndex } from '../data/index.js'
import { el } from './components.ts'
import { statusOf } from './list-model.ts'

/** The TUI's step: one spinner frame, and one cell of the band, every 120 ms (`SPIN_TICK`). */
const STEP_MS = 120
/** The band's cells, and the cells it rests past the end before it comes in again (`glint()`, `GLINT_REST`). */
const BAND = 3
const REST = 8

/**
 * Does the row at `at` move? The TUI's rule (`Config::is_started`): a column that is neither the first nor done is a
 * started one — in progress, review, and a column of the project's own between them — so review moves too. A group's
 * status is the one derived from its members (statusOf). Deferred work stands still. `first` is the project's first
 * configured column (`/api/boot`'s `statuses[0]`); until boot answers, moai's default.
 */
export function spins(index: IssueIndex, at: number, first: string = FIRST): boolean {
  const status = statusOf(index.rows[at]!)
  return status !== '' && status !== first && status !== 'done' && index.shelvedAt(at) === undefined
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
  return el(doc, 'span', { class: 'm-glint', style: `--glint-time: ${glintTime(text)}ms;` }, ...(content.length ? content : [text]))
}
