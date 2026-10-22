/**
 * The TUI's branch mark (`⎇ <branch>`, bright cyan, bold) on a row another git worktree holds (se-etwu). It reads
 * what `show --all --worktree` puts on the row: `working`, the worktree that holds the issue (the TUI's
 * `Origin::working`), when moai gives it, else `branch`, the worktree the row came from (`Origin::branch`). A done row
 * gets none: a worktree merged and not cleaned up keeps its name after the work ends (moai-9a8m).
 */
import type { Row } from '../data/index.js'
import { el } from './components.ts'
import { statusOf } from './list-model.ts'

export const BRANCH_GLYPH = '⎇'
/** The TUI's `BRANCH_CAP`: a long branch name keeps its tail, where its issue id is. */
export const BRANCH_CAP = 16

const text = (value: unknown): string | undefined => (typeof value === 'string' && value !== '' ? value : undefined)

/** Keep the last `cap` letters, the first of them `…`, as the TUI's `clip_front`. */
export function clipFront(name: string, cap = BRANCH_CAP): string {
  const letters = [...name]
  return letters.length <= cap ? name : `…${letters.slice(letters.length - (cap - 1)).join('')}`
}

/** The worktree branch a row is held in, or undefined. */
export function branchOf(row: Row): string | undefined {
  if (statusOf(row) === 'done') return undefined
  return text(row.working) ?? text(row.branch)
}

/** `⎇ <branch>`, the name clipped at the front; the whole name in its title. */
export function branchMark(doc: Document, branch: string): HTMLElement {
  return el(doc, 'span', { class: 'm-branch', title: `${BRANCH_GLYPH} ${branch}` }, `${BRANCH_GLYPH} ${clipFront(branch)}`)
}

/** The worktrees whose rows stand in the list: the path line names them, as the TUI's does. */
export function worktreesOf(rows: readonly Row[]): string[] {
  return [...new Set(rows.map((row) => text(row.branch)).filter((name): name is string => name !== undefined))].sort()
}
