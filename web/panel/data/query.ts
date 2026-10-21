/**
 * The filter box's helps and the sort editor's words. **The meaning of a filter, a search and an order is moai's**:
 * the page sends the filter line as typed to `/api/rows?filter=…` (moai cuts it with `query::split_items` and judges
 * it with `query::Filter`), the search to `?q=…` (`show --grep`, which reads titles, bodies and notes), and the order
 * to `/api/tree?sort=…` (`query::parse_order`). A filter moai refuses comes back as its own `bad_filter` words.
 *
 * What stays here only helps a person type: {@link items} and {@link slotAt} find the item under the caret, and
 * {@link hintsAt} offers keys and values from the rows on screen. A hint is a suggestion — the server judges.
 * The words — the filter keys and the sort fields with their own directions — come from `/api/boot`
 * ({@link useVocabulary}); the lists below are what stands before it answers.
 */
import { parentOf, priorityOf, type Row } from './index.js'

// ---- the words ------------------------------------------------------------------------------------------

/** Filter keys as moai 0.20 names them (`query::KEYS`), until `/api/boot` says. */
let filterKeys: readonly string[] = ['status', 'tag', 'no-tag', 'epic', 'milestone', 'parent', 'priority', 'assignee', 'type', 'grep', 'stale', 'since', 'created', 'done', 'due_on', 'created_at', 'updated_at', 'started_at', 'done_at']

/** The fields a list may be ordered by (`query::SortKey::ALL`). */
export type SortKey = 'priority' | 'created' | 'updated' | 'status' | 'assignee' | 'title' | 'due' | 'id'
/** The sort fields in the order moai names them (also the order the sort editor lists them). */
export const SORT_KEYS: readonly SortKey[] = ['priority', 'created', 'updated', 'status', 'assignee', 'title', 'due', 'id']
export type Dir = 'asc' | 'desc'
/** Each field's own direction (`SortKey::default_dir`), until `/api/boot` says. */
const ownDir = new Map<SortKey, Dir>([['created', 'desc'], ['updated', 'desc']])

/** Takes the words `/api/boot` gives: the filter keys, and each sort field's own direction. */
export function useVocabulary(boot: { filter_keys: readonly string[]; sort_fields: readonly { name: string; dir: Dir }[] }): void {
  if (boot.filter_keys.length) filterKeys = [...boot.filter_keys]
  for (const field of boot.sort_fields) {
    const key = SORT_KEYS.find((one) => one === field.name)
    if (key) ownDir.set(key, field.dir)
  }
}

/** The filter keys the hints offer. */
export const filterKeyList = (): readonly string[] => filterKeys

// ---- the line -------------------------------------------------------------------------------------------

/** An item of filter text with where it stands. Offsets are UTF-16 indexes into the text. */
export interface Item {
  /** What the filter reads: quotes stripped, later bare words joined to it with one space. */
  text: string
  /** First index of the word that opened the item. */
  start: number
  /** Index of its first `=`; none for a leading word without one. */
  eq?: number
  /** End of the opening word (past the closing quote for a quoted value; the text's end if never closed). */
  end: number
  /** Indexes of the quote right after `=` and of its closing quote (absent while unclosed). */
  quote?: { open: number; close?: number }
}

const isSpace = (c: string): boolean => /\s/.test(c)

/**
 * Cuts a typed filter line into items, as moai's `query::split_items` does — the meaning is that one; this copy
 * only finds where the caret is for the hints and the chips. An item starts at a word with `=`; a word without one
 * joins the item before it with a single space, so a value may hold spaces. A quote right after `=` holds one value
 * (quotes stripped; an unclosed one runs to the end of the line). Quotes elsewhere are plain letters.
 */
export function items(text: string): Item[] {
  const out: Item[] = []
  let at = 0
  while (true) {
    while (at < text.length && isSpace(text.charAt(at))) at++
    if (at >= text.length) return out
    const start = at
    let word = ''
    let eq: number | undefined
    let quote: Item['quote']
    while (at < text.length && !isSpace(text.charAt(at))) {
      const c = text[at]
      word += c
      at++
      if (c === '=' && eq === undefined) {
        eq = at - 1
        if (at < text.length && (text[at] === '"' || text[at] === "'")) {
          const q = text[at]
          const open = at
          at++
          let close: number | undefined
          while (at < text.length) {
            if (text[at] === q) {
              close = at
              at++
              break
            }
            word += text[at]
            at++
          }
          quote = close === undefined ? { open } : { open, close }
        }
      }
    }
    const last = out[out.length - 1]
    if (last && eq === undefined) last.text += ` ${word}`
    else out.push({ text: word, start, eq, end: at, quote })
  }
}

/** The item texts of a typed line. */
export const splitItems = (text: string): string[] => items(text).map((item) => item.text)

// ---- sort -----------------------------------------------------------------------------------------------

/** One field of an order and its direction (SQL's `ORDER BY` item). */
export interface SortField {
  key: SortKey
  dir: Dir
}
/** An order: its fields (the next one decides only where the ones before it tie) and a whole-list reverse. */
export interface Sort {
  fields: SortField[]
  reversed: boolean
}

/** The direction a field has when none is written. */
export const defaultDir = (key: SortKey): Dir => ownDir.get(key) ?? 'asc'
/** A field in its own direction. */
export const fieldOf = (key: SortKey): SortField => ({ key, dir: defaultDir(key) })
/** The order moai shows when none is chosen: priority, then id. */
export const DEFAULT_SORT: Sort = { fields: [{ key: 'priority', dir: 'asc' }], reversed: false }

/** Writes fields as `--sort` reads them: the key alone in its own direction, else `key:asc|desc`. */
export const formatSort = (fields: readonly SortField[]): string =>
  fields.map((field) => (field.dir === defaultDir(field.key) ? field.key : `${field.key}:${field.dir}`)).join(',')

/** The same order: the same fields in the same directions, and the same reverse switch. */
export const sameSort = (a: Sort, b: Sort): boolean => a.reversed === b.reversed && formatSort(a.fields) === formatSort(b.fields)

/** The query `/api/tree` takes for an order: every direction written out, so the server never guesses one. */
export function sortQuery(sort: Sort): Record<string, string> {
  const query: Record<string, string> = {}
  if (sort.fields.length) query.sort = sort.fields.map((field) => `${field.key}:${field.dir}`).join(',')
  if (sort.reversed) query.reverse = '1'
  return query
}

/** Choose one field alone, in its own direction (a plain key press); the reverse switch is cleared. */
export const sortReplace = (key: SortKey): Sort => ({ fields: [fieldOf(key)], reversed: false })

/** Add a field at the end of the order. A field already in it stays where it is. */
export const sortAppend = (sort: Sort, key: SortKey): Sort =>
  sort.fields.some((field) => field.key === key) ? sort : { ...sort, fields: [...sort.fields, fieldOf(key)] }

/** Turn one field's direction around. A field not yet in the order is appended in its own direction. */
export const sortFlip = (sort: Sort, key: SortKey): Sort =>
  sort.fields.some((field) => field.key === key)
    ? { ...sort, fields: sort.fields.map((field) => (field.key === key ? { key, dir: field.dir === 'asc' ? 'desc' : 'asc' } : field)) }
    : sortAppend(sort, key)

/** Turn the whole list around, ties included. */
export const sortReverse = (sort: Sort): Sort => ({ ...sort, reversed: !sort.reversed })

/** How a key press edits the order: `replace` (alone), `append` (to the end), `flip` (direction). */
export type SortEdit = 'replace' | 'append' | 'flip'

/**
 * One key press on a field. `replace` picks it alone, except when it already is the single field: then its
 * direction is turned (pressing the same key twice reverses it). `append` and `flip` are as above.
 */
export function sortEdit(sort: Sort, key: SortKey, how: SortEdit): Sort {
  if (how === 'append') return sortAppend(sort, key)
  if (how === 'flip') return sortFlip(sort, key)
  return sort.fields.length === 1 && sort.fields[0]?.key === key ? sortFlip(sort, key) : sortReplace(key)
}

/** What a sort badge shows, as data: one part per field, in order, plus the switch and the written form. */
export interface SortBadge {
  parts: { key: SortKey; dir: Dir; /** The direction is not the key's own. */ explicit: boolean }[]
  reversed: boolean
  /** The order is the default one (priority alone, not reversed): a badge may stay quiet. */
  isDefault: boolean
  /** `priority,updated:desc` as `--sort` takes it. */
  text: string
}

/** The data of the sort badge for an order. */
export function sortBadge(sort: Sort): SortBadge {
  return {
    parts: sort.fields.map((field) => ({ key: field.key, dir: field.dir, explicit: field.dir !== defaultDir(field.key) })),
    reversed: sort.reversed,
    isDefault: !sort.reversed && formatSort(sort.fields) === 'priority',
    text: formatSort(sort.fields),
  }
}

// ---- filter hints ---------------------------------------------------------------------------------------

/** A candidate for a value (or a key): what to insert, how many rows carry it, a label to show beside it. */
export interface Hint {
  value: string
  count: number
  label?: string
}

/** What the value hints read beyond the rows: the columns in the config's order. */
export interface HintContext {
  statuses?: readonly string[]
}

/** Filter keys that start with `prefix` (all of them when empty), in moai's order. */
export const keyHints = (prefix = ''): string[] => filterKeys.filter((key) => key.startsWith(prefix.trim().toLowerCase()))

const KINDS = ['issue', 'epic', 'milestone', 'backlog'] as const

/** Code point order, which is what moai's UTF-8 byte order comes to. */
const compareCodePoints = (a: string, b: string): number => {
  const x = Array.from(a)
  const y = Array.from(b)
  for (let i = 0; i < Math.min(x.length, y.length); i++) {
    const d = (x[i]?.codePointAt(0) ?? 0) - (y[i]?.codePointAt(0) ?? 0)
    if (d !== 0) return d
  }
  return x.length - y.length
}

const str = (value: unknown): string | undefined => (typeof value === 'string' && value !== '' ? value : undefined)
const unassigned = (row: Row): boolean => row.assignee === undefined || row.assignee === null || String(row.assignee).trim() === ''
const isGroupKind = (row: Row): boolean => row.kind === 'epic' || row.kind === 'milestone'
const kindName = (row: Row): string => (row.kind === 'idea' ? 'backlog' : KINDS.find((kind) => kind === row.kind) ?? 'issue')

const tally = (values: Iterable<string>): Map<string, number> => {
  const counts = new Map<string, number>()
  for (const value of values) counts.set(value, (counts.get(value) ?? 0) + 1)
  return counts
}
const byCount = (a: Hint, b: Hint): number => b.count - a.count || compareCodePoints(a.value, b.value)

/**
 * Value candidates for `key` from the rows: tags (`tag`, `no-tag`), assignee names, epics and milestones
 * (id with the title as label, plus `none`), parents, columns (in the config's order), priorities, kinds. The
 * rows say where they stand themselves (`derived_status`, `derived_epic`, `derived_milestone`). Narrowed by `prefix`
 * against the value and its label, case-folded (a value that begins with it first); most-carried first.
 */
export function valueHints(rows: readonly Row[], key: string, prefix = '', ctx: HintContext = {}): Hint[] {
  const needle = prefix.trim().toLowerCase()
  let hints: Hint[] = []
  const none = (count: number): Hint[] => (count > 0 ? [{ value: 'none', count }] : [])
  const named = (kind: 'epic' | 'milestone', of: (row: Row) => string | undefined): Hint[] => {
    const counts = tally(rows.flatMap((row) => of(row) ?? []))
    const owners = rows.filter((row) => row.kind === kind)
    const listed = owners.map((owner): Hint => ({ value: owner.id, count: counts.get(owner.id) ?? 0, label: owner.title }))
    for (const [id, count] of counts) if (!owners.some((owner) => owner.id === id)) listed.push({ value: id, count })
    return [...listed.sort(byCount), ...none(rows.filter((row) => !isGroupKind(row) && of(row) === undefined).length)]
  }
  switch (key) {
    case 'tag':
    case 'no-tag':
      hints = [...tally(rows.flatMap((row) => (Array.isArray(row.tags) ? (row.tags as string[]) : [])))]
        .map(([value, count]) => ({ value, count }))
        .sort(byCount)
      break
    case 'assignee':
      hints = [...tally(rows.filter((row) => !unassigned(row)).map((row) => String(row.assignee).trim()))]
        .map(([value, count]) => ({ value, count }))
        .sort(byCount)
        .concat(none(rows.filter(unassigned).length))
      break
    case 'epic':
      hints = named('epic', (row) => str(row.derived_epic))
      break
    case 'milestone':
      hints = named('milestone', (row) => str(row.derived_milestone))
      break
    case 'parent': {
      const counts = tally(rows.flatMap((row) => parentOf(row.id) ?? []))
      hints = [...counts].map(([value, count]) => ({ value, count, label: rows.find((row) => row.id === value)?.title })).sort(byCount)
      hints.push(...none(rows.filter((row) => parentOf(row.id) === undefined).length))
      break
    }
    case 'status': {
      const statuses = ctx.statuses ?? []
      const rank = (value: string): number => (statuses.indexOf(value) < 0 ? statuses.length : statuses.indexOf(value))
      hints = [...tally(rows.map((row) => str(row.derived_status) ?? str(row.status) ?? '').filter((value) => value !== ''))]
        .map(([value, count]) => ({ value, count }))
        .sort((a, b) => rank(a.value) - rank(b.value) || compareCodePoints(a.value, b.value))
      break
    }
    case 'priority': {
      const counts = tally(rows.map((row) => String(priorityOf(row))))
      hints = ['0', '1', '2', '3'].map((value) => ({ value, count: counts.get(value) ?? 0 }))
      break
    }
    case 'type': {
      const counts = tally(rows.map(kindName))
      hints = KINDS.map((value) => ({ value, count: counts.get(value) ?? 0 }))
      break
    }
    default:
      return []
  }
  if (needle === '') return hints
  const first = (hint: Hint): boolean => hint.value.toLowerCase().startsWith(needle)
  const found = hints.filter((hint) => hint.value.toLowerCase().includes(needle) || (hint.label ?? '').toLowerCase().includes(needle))
  // Those that begin with what was typed come first; each group keeps its order.
  return [...found.filter(first), ...found.filter((hint) => !first(hint))]
}

const TIME_KEYS: readonly string[] = ['since', 'created', 'done', 'due_on', 'created_at', 'updated_at', 'started_at', 'done_at']

/** The item under the cursor of a filter line: which key it is on and the value typed so far. */
export interface Slot {
  /** The item's key, once it has an `=` before the cursor; absent while a key is still being typed. */
  key?: string
  /** The key typed so far (no `=` yet), or the value piece before the cursor (after the last comma). */
  partial: string
  /** The range of `partial` in the line, for replacing it with a chosen hint. */
  from: number
  to: number
}

/** Finds what the cursor (a UTF-16 index) is typing in a filter line. A cursor in blank space starts a new key. */
export function slotAt(text: string, cursor: number): Slot {
  const list = items(text)
  // An item reaches over the bare words joined to it, up to where the next item starts.
  const reach = (at: number): number => {
    const next = list[at + 1]
    return (list[at]?.start ?? 0) + text.slice(list[at]?.start ?? 0, next ? next.start : text.length).trimEnd().length
  }
  const item = list.find((one, at) => cursor >= one.start && cursor <= reach(at))
  if (!item) return { partial: '', from: cursor, to: cursor }
  if (item.eq === undefined || cursor <= item.eq) {
    // Only a leading bare word is a key being typed; a later one is part of the previous item's value.
    return { partial: text.slice(item.start, cursor), from: item.start, to: cursor }
  }
  const key = text.slice(item.start, item.eq).trim()
  const open = item.quote?.open
  const valueStart = open === undefined ? item.eq + 1 : open + 1
  const upto = text.slice(valueStart, cursor)
  // A bare word after a value is joined to it, but only `grep` and the time keys take values with a space
  // in them: for any other key the word is the start of the next key.
  const spaced = open === undefined && /\s/.test(upto) && key !== 'grep' && !TIME_KEYS.includes(key)
  if (spaced) {
    const word = upto.slice(upto.search(/\S+$/))
    return { partial: word, from: cursor - word.length, to: cursor }
  }
  const comma = upto.lastIndexOf(',')
  const from = comma < 0 ? valueStart : valueStart + comma + 1
  return { key, partial: text.slice(from, cursor).trimStart(), from, to: cursor }
}

/** The hints for the cursor of a filter line: keys while a key is typed (adding `=`), values after it. */
export function hintsAt(rows: readonly Row[], text: string, cursor: number, ctx: HintContext = {}): { slot: Slot; hints: Hint[] } {
  const slot = slotAt(text, cursor)
  const hints =
    slot.key === undefined
      ? keyHints(slot.partial).map((value): Hint => ({ value, count: 0 }))
      : valueHints(rows, slot.key, slot.partial, ctx)
  return { slot, hints }
}
