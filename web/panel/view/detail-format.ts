/**
 * What the detail pane says, as plain data: the head line, the labeled fields, a blocker's state, a group's
 * roll-up, and one line each for a commit, a journal entry and a work record. No DOM here; view/detail.ts
 * draws it and tests/moai-e3-detail.test.mjs checks it.
 */
import { isDone, isGroup, type IssueIndex, type Progress, type Row } from '../data/index.ts'
import type { Outcome } from '../data/client.ts'
import { notStartedText, type Copy } from '../i18n/index.ts'

/** The columns in moai's default order; a column the config adds stands after them. */
const STATUS_ORDER = ['todo', 'in_progress', 'review', 'done']

const isRecord = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null && !Array.isArray(value)
const text = (value: unknown): string => (typeof value === 'string' ? value : typeof value === 'number' ? String(value) : '')
const strings = (value: unknown): string[] => (Array.isArray(value) ? value.filter((item): item is string => typeof item === 'string' && item !== '') : [])

/** `YYYY-MM-DD HH:MM` in the host's time zone; '' for anything that is not a date. */
export function formatDate(value: unknown): string {
  // A bare date (a due day) has no time zone to move it into.
  if (typeof value === 'string' && /^\d{4}-\d{2}-\d{2}$/.test(value)) return value
  const date = new Date(typeof value === 'string' ? value : NaN)
  if (Number.isNaN(date.getTime())) return ''
  const two = (n: number) => String(n).padStart(2, '0')
  return `${date.getFullYear()}-${two(date.getMonth() + 1)}-${two(date.getDate())} ${two(date.getHours())}:${two(date.getMinutes())}`
}

export type Tone = 'neutral' | 'ok' | 'warn' | 'danger' | 'accent'

/** The status chip's tone: done is calm, work in flight is accent, review asks for attention. */
export function statusTone(status: unknown): Tone {
  if (status === 'done') return 'ok'
  if (status === 'in_progress') return 'accent'
  if (status === 'review') return 'warn'
  return 'neutral'
}

export interface Head {
  id: string
  title: string
  status: string
  statusLabel: string
  tone: Tone
  /** `P2`, or '' when the row carries none. */
  priority: string
  /** `epic`, `task`… — the kind's name; '' for the common case of a plain task. */
  kind: string
  /** Why it is put off (the reason, else the day), or '' when it is not. */
  deferred: string
  /** The row that put it off when it stands under a deferred epic, milestone or parent (`shelved_by`), else ''. */
  shelvedBy: string
}

export function headOf(row: Row, copy: Copy): Head {
  // A group's own state is read from its members; `derived_status` is what the list shows for it.
  const status = text(isGroup(row) ? row.derived_status ?? row.status : row.status)
  const kind = text(row.kind)
  return {
    id: row.id,
    title: text(row.title) || row.id,
    status,
    statusLabel: copy.stats[status] ?? status,
    tone: statusTone(status),
    priority: typeof row.priority === 'number' ? copy.detail.priority(row.priority) : '',
    kind: kind && kind !== 'issue' ? copy.detail.kinds[kind] ?? kind : '',
    deferred: text(row.deferred_at) && !isDone(row) ? text(row.deferred_reason) || formatDate(row.deferred_at) || copy.detail.deferred : '',
    shelvedBy: !isDone(row) ? text(row.shelved_by) : '',
  }
}

export type BlockerState = 'done' | 'open' | 'deferred' | 'missing'

/** What `show --json` says of each blocker: `blockers: [{id, state}]` (open, deferred, done, missing, empty). */
function blockerStates(row: Row | undefined): Map<string, BlockerState> {
  const out = new Map<string, BlockerState>()
  for (const entry of Array.isArray(row?.blockers) ? (row!.blockers as unknown[]) : []) {
    if (!isRecord(entry) || typeof entry.id !== 'string') continue
    const state = entry.state
    // An empty group blocks until it has members: it reads as open.
    if (state === 'done' || state === 'open' || state === 'deferred' || state === 'missing') out.set(entry.id, state)
    else if (state === 'empty') out.set(entry.id, 'open')
  }
  return out
}

/**
 * One blocker: ✓ finished, · open, put off, or an id nothing holds. `show` answers for each blocker itself
 * (`row.blockers`, read from the row the detail draws); the index is the fallback for a row without that key,
 * and the panel fills it with open work only — so a finished blocker is only ever told right by `show`.
 */
export function blockerState(id: string, index: IssueIndex | null, row?: Row): BlockerState {
  const given = blockerStates(row).get(id)
  if (given) return given
  const at = index?.find(id)
  if (at === undefined) return 'missing'
  const found = index!.rows[at]!
  // A group's column is read from its members (derived_status), as the list does; its stored status is not read.
  if (isGroup(found) ? (found.derived_status ?? found.status) === 'done' : isDone(found)) return 'done'
  return index!.shelvedAt(at) !== undefined ? 'deferred' : 'open'
}

export const BLOCKER_MARK: Record<BlockerState, string> = { done: '✓', open: '·', deferred: '⏸', missing: '?' }

/** What blocks the row, from its own `blocked_by`. */
export const blockedByOf = (row: Row): string[] => strings(row.blocked_by)

/** The rows that name this one in their `blocked_by`, plus the ids `show` says it blocks. */
export function blocksOf(row: Row, index: IssueIndex | null): string[] {
  const out = new Set(strings(row.blocks))
  for (const other of index?.rows ?? []) if (blockedByOf(other).includes(row.id)) out.add(other.id)
  return [...out]
}

export interface Rollup {
  done: number
  work: number
  percent: number | undefined
  /** Counts per status of the work rows, in the order the statuses first appear. */
  byStatus: [string, number][]
}

/**
 * `n/n` for a group, the count the list draws: `index.tally()`, which for a milestone or epic is the one `moai
 * status` gave (archived done members included). The per-status counts are the rows the index holds, with the done
 * members it does not (the archive's) counted as done. Null for anything that is not a group.
 */
export function rollupOf(row: Row, index: IssueIndex | null): Rollup | null {
  if (!index) return null
  const at = index.find(row.id)
  // A group, or an issue with `.child` ids — whatever the list draws as a folder with an n/n.
  if (at === undefined || !(isGroup(row) || index.isDir(at))) return null
  const path = index.dirPath(at)
  const progress: Progress = index.progress(path)
  const [done, total] = index.tally(path)
  const counts = new Map<string, number>()
  for (const work of progress.work) {
    const status = text(index.rows[work]!.status) || 'todo'
    counts.set(status, (counts.get(status) ?? 0) + 1)
  }
  const unheld = done - progress.done
  if (unheld > 0) counts.set('done', (counts.get('done') ?? 0) + unheld)
  // The columns in their order (todo, in_progress, review, done), then any other in the order met.
  const rank = (status: string) => {
    const at = STATUS_ORDER.indexOf(status)
    return at < 0 ? STATUS_ORDER.length : at
  }
  const byStatus = [...counts].sort((a, b) => rank(a[0]) - rank(b[0]))
  return { done, work: total, percent: total ? Math.floor((done * 100) / total) : undefined, byStatus }
}

export interface Field {
  key: string
  label: string
  value: string
}

/** The labeled fields under the head line, in the TUI's order; one that has no value is left out. */
export function fieldsOf(row: Row, index: IssueIndex | null, copy: Copy): Field[] {
  const d = copy.detail
  const epic = text(row.epic) || text(row.derived_epic)
  const milestone = text(row.milestone)
  const titled = (id: string) => {
    const at = index?.find(id)
    const title = at === undefined ? '' : text(index!.rows[at]!.title)
    return title ? `${title} (${id})` : id
  }
  const assignee = text(row.assignee)
  const email = text(row.assignee_email)
  const all: [string, string, string][] = [
    ['assignee', d.assignee, assignee && email && email !== assignee ? `${assignee} <${email}>` : assignee || email],
    ['epic', d.epic, epic ? titled(epic) : ''],
    ['milestone', d.milestone, milestone ? titled(milestone) : ''],
    // The CLI's key is `due_on` (and `starts_on`); `due` is what an older row may carry.
    ['due', d.due, formatDate(row.due_on ?? row.due)],
    ['created', d.created, formatDate(row.created_at)],
    ['updated', d.updated, formatDate(row.updated_at)],
    ['started', d.started, formatDate(row.started_at)],
    ['ended', d.ended, formatDate(row.done_at ?? row.ended_at)],
  ]
  return all.filter(([, , value]) => value).map(([key, label, value]) => ({ key, label, value }))
}

export const tagsOf = (row: Row): string[] => strings(row.tags)

/** A commit as `show` prints it: a bare hash, or an object with a hash and a subject. */
export function commitLine(item: unknown): { hash: string; subject: string } | null {
  if (typeof item === 'string') return item ? { hash: item.slice(0, 7), subject: '' } : null
  if (!isRecord(item)) return null
  const hash = text(item.hash) || text(item.sha) || text(item.commit) || text(item.id)
  const subject = text(item.subject) || text(item.title) || text(item.message)
  return hash || subject ? { hash: hash.slice(0, 7), subject: subject.split('\n')[0]! } : null
}

export interface JournalLine {
  /** `create`, `note`, `status`, or whatever moai wrote. */
  kind: string
  text: string
  meta: string
}

/** One journal entry: what happened, then who and when. */
export function journalLine(entry: unknown, copy: Copy): JournalLine | null {
  if (!isRecord(entry)) return null
  const kind = text(entry.kind)
  let said: string
  if (kind === 'create') said = copy.detail.created_entry
  else if (kind === 'status') {
    const from = text(entry.from)
    const to = text(entry.to)
    // `mv -m` leaves a note on the move; the CLI prints it after a dash.
    const note = text(entry.note)
    said = `${copy.stats[from] ?? from} → ${copy.stats[to] ?? to}${note ? ` — ${note}` : ''}`
  } else said = text(entry.text) || kind
  return { kind, text: said, meta: [text(entry.by), formatDate(entry.ts)].filter(Boolean).join(' · ') }
}

/** The journal as the file keeps it: oldest first, so it reads top to bottom. */
export function journalOf(row: Row, copy: Copy): JournalLine[] {
  return (Array.isArray(row.journal) ? row.journal : []).map((entry) => journalLine(entry, copy)).filter((line): line is JournalLine => line !== null)
}

export function commitsOf(row: Row): { hash: string; subject: string }[] {
  // `show --json` sends every commit and flags the tracker's own (`chore(tracker): …`) with `tracker: true`;
  // the receiver decides, and moai's own views leave them out.
  return (Array.isArray(row.commits) ? row.commits : [])
    .filter((item) => !(isRecord(item) && item.tracker === true))
    .map(commitLine)
    .filter((line): line is { hash: string; subject: string } => line !== null)
}

/**
 * A work record as one line. moai's is {provider, model, tokens, grade, why, at, by}: it reads like the note
 * it was written from, `anthropic/opus-5 tokens=12345 (medium — wrote the list view)`, then who and when.
 * A record of another shape takes what reads as text.
 */
export function workLine(item: unknown): string {
  if (typeof item === 'string') return item
  if (!isRecord(item)) return ''
  const model = [text(item.provider), text(item.model)].filter(Boolean).join('/')
  const tokens = typeof item.tokens === 'number' && Number.isFinite(item.tokens) ? `tokens=${item.tokens}` : ''
  const verdict = [text(item.grade), text(item.why)].filter(Boolean).join(' — ')
  const recorded = [model, tokens, verdict ? `(${verdict})` : ''].filter(Boolean).join(' ')
  const main = recorded || text(item.summary) || text(item.text) || text(item.title) || text(item.note) || text(item.kind)
  const who = text(item.by) || text(item.agent) || (recorded ? '' : text(item.model))
  const when = formatDate(item.ts ?? item.at)
  return [main, [who, when].filter(Boolean).join(' · ')].filter(Boolean).join('  ')
}

export const workOf = (row: Row): string[] => (Array.isArray(row.work) ? row.work : []).map(workLine).filter(Boolean)

/** The subtasks `show` lists under the row: ids (what the CLI prints), or rows; a title comes from the index. */
export function childrenOf(row: Row, index: IssueIndex | null = null): { id: string; title?: string }[] {
  const out: { id: string; title?: string }[] = []
  for (const child of Array.isArray(row.children) ? row.children : []) {
    const id = typeof child === 'string' ? child : isRecord(child) && typeof child.id === 'string' ? child.id : ''
    if (!id) continue
    const given = isRecord(child) ? text(child.title) : ''
    const at = index?.find(id)
    const title = given || (at === undefined ? '' : text(index!.rows[at]!.title))
    out.push(title ? { id, title } : { id })
  }
  return out
}

/** Why a read failed, in the words the shell uses for the same outcome; null for one that passed. */
export function problemText(outcome: Outcome, copy: Copy): string | null {
  switch (outcome.kind) {
    case 'refused': return copy.refused[outcome.code]
    case 'not-initialized': return `${copy.notInitialized[0]}moai init${copy.notInitialized[1]}`
    case 'moai-error': return copy.moaiError(outcome.message)
    case 'cancelled': return outcome.mayContinue ? `${copy.cancelled} ${copy.mayContinue}` : copy.cancelled
    case 'unreadable': return copy.unreadable
    case 'not-started': return notStartedText(copy, outcome.code)
    case 'failed': return copy.failed
    case 'passed': return null
  }
}
