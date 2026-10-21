/**
 * Filter and sort over rows, ported from moai's `query.rs` (the parts the panel's list needs). No DOM here;
 * `node --test` checks it against `moai show --filter/--sort --json` output on a fixture tracker.
 *
 * One sentence of grammar: **a comma is "or", a repeated key is "and", different keys are "and".** The one
 * exception is `assignee`, where a repeat is "or" too. A filter is `item=value` words (`status=todo
 * tag=bug,ui`); {@link items} cuts a typed line into items the way the explorer's filter box does.
 *
 * What the rows alone cannot answer is read from their JSON fields: the column of a group is its
 * `derived_status`, its epic is `derived_epic`, its milestone comes through that epic. A deferral is
 * inherited as the {@link IssueIndex} walks it (id parents, epics, milestones), built from the rows on first
 * use. Whoever holds an index already may pass its answers in {@link Context} instead.
 *
 * Not ported (see the report of the change that added this file): the eclipsed/lost-row rules that need
 * duplicate ids, the journal-only filters (`--removed`), keyset paging (`--after`) and `me`.
 */
import { DEFAULT_STATUSES, IssueIndex, displayOrder, isBacklog, isGroup, parentOf, priorityOf, type Row } from './index.js'

/** Everything the filter and the order may need beyond the rows themselves. All of it is optional. */
export interface Context {
  /** "Now" as `YYYY-MM-DDTHH:MM:SSZ`, for `stale` and the archive. Absent: those two never hold. */
  now?: string
  /** The column order of the tracker's config (`statuses`); a column not listed sorts last. */
  statuses?: readonly string[]
  /** How a person is shown, as the tracker's `naming`: the assignee order sorts by what the screen shows. */
  naming?: 'full' | 'name' | 'email'
  /** Notes by issue id (`moai note` texts, not in the row). Without them the note scope finds nothing. */
  notes?: Readonly<Record<string, readonly string[]>>
  /** Days a row stands in done before it is the archive; 0 or absent turns it off (the CLI's `archive_days`). */
  archiveDays?: number
  /** Epoch seconds of an instant to the reader's wall clock (adds the zone offset); absent: UTC. */
  local?: (epoch: number) => number
  /** Overrides of what is otherwise read from the row (and, through `derived_epic`, from its epic row). */
  epicOf?: (row: Row) => string | undefined
  milestoneOf?: (row: Row) => string | undefined
  deferredOf?: (row: Row) => boolean
}

export type Result<T, E> = { ok: true; value: T } | { ok: false; error: E }

// ---- filter ---------------------------------------------------------------------------------------------

/** The filter items, in the order moai lists them. An unknown key is refused with this list. */
export const KEYS = [
  'status',
  'tag',
  'no-tag',
  'epic',
  'milestone',
  'parent',
  'priority',
  'assignee',
  'type',
  'grep',
  'stale',
  'since',
  'created',
  'done',
  'created_at',
  'updated_at',
  'started_at',
  'done_at',
] as const
export type Key = (typeof KEYS)[number]

/** Where `grep` looks. `all` sees the other five; `note` comes last, after the four fields of the row. */
export type GrepScope = 'all' | 'id' | 'title' | 'tag' | 'body' | 'note'
/**
 * The scopes the panel offers, in the order Tab cycles them. `note` is left out: the rows carry no notes and
 * nothing loads them into {@link Context.notes}, so it would find nothing and say nothing (se-thyl). The matcher
 * still reads notes when given; the scope comes back once a source fills them (moai 0.17 `show -g` sees notes).
 */
export const GREP_SCOPES: readonly GrepScope[] = ['all', 'id', 'title', 'tag', 'body']

/** Next scope (Tab), wrapping at the end. */
export const nextScope = (scope: GrepScope): GrepScope => GREP_SCOPES[(GREP_SCOPES.indexOf(scope) + 1) % GREP_SCOPES.length] ?? 'all'
/** Previous scope (Shift-Tab), wrapping at the start. */
export const prevScope = (scope: GrepScope): GrepScope => GREP_SCOPES[(GREP_SCOPES.indexOf(scope) + GREP_SCOPES.length - 1) % GREP_SCOPES.length] ?? 'all'
/** Whether the scope reads the row's id, title, tags or body (a hit is painted there). */
export const sees = (scope: GrepScope, part: 'id' | 'title' | 'tag' | 'body' | 'note'): boolean => scope === 'all' || scope === part

/** `epic=none` style slot: the row has none (`unset`) or this exact id. */
export type Sel = { unset: true } | { unset: false; is: string }

/** One span of time, both ends inclusive; a missing end is open. `wall` ends are read on the reader's clock. */
export interface Span {
  from?: { at: number; wall: boolean }
  to?: { at: number; wall: boolean }
}

/** A built filter. Fields are the compiled form of `item=value`; an empty list means "anything". */
export interface Filter {
  status: string[]
  /** AND of OR: the outer list is "and", each inner list "or". */
  tags: string[][]
  noTags: string[]
  epic: Sel[]
  milestone: Sel[]
  parent: Sel[]
  priority: number[]
  assignee: Sel[]
  kind?: 'issue' | 'epic' | 'milestone' | 'backlog'
  /** Lowercased text to find. */
  grep?: string
  grepIn: GrepScope
  stale?: number
  /** AND of OR, like `tags`. */
  updated: Span[][]
  created: Span[][]
  done: Span[][]
  started: Span[][]
  doneAt: Span[][]
  /** Let done and deferred rows in (not the archive). */
  all: boolean
  /** Let the archive (old done rows) in. */
  archived: boolean
  /** `true`: only deferred rows; `false`: only rows not deferred; absent: deferred rows are hidden. */
  deferred?: boolean
  /** Let backlog rows in. */
  backlog: boolean
}

/** Why a filter text was refused. Data only; {@link badFilterMessage} words it. */
export type BadFilter =
  | { code: 'twice'; key: Key; values: string[] }
  | { code: 'done-outside'; asked: string }
  | { code: 'endless'; value: string }
  | { code: 'backwards'; value: string }
  | { code: 'not-a-time'; value: string }
  | { code: 'no-time' }
  | { code: 'not-a-pair'; value: string }
  | { code: 'no-such-key'; key: string }
  | { code: 'not-days'; value: string }
  | { code: 'not-a-priority'; value: string }
  | { code: 'not-a-kind'; value: string }

/** The switches that sit beside the filter text (the CLI's flags); a time question opens them all. */
export interface Flags {
  /** Include done and deferred rows. */
  all?: boolean
  /** Include the archive too (implies `all`). */
  archived?: boolean
  /** Include backlog rows. */
  backlog?: boolean
  /** Keep only deferred rows. */
  deferred?: boolean
  /** Where a `grep=` item looks; `all` when absent. */
  grepIn?: GrepScope
}

const single: readonly Key[] = ['grep', 'type', 'stale']
const once: readonly Key[] = ['status', 'epic', 'milestone', 'parent', 'priority']

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
 * Cuts a typed filter line into items. An item starts at a word with `=`; a word without one joins the
 * item before it with a single space, so a value may hold spaces. A quote right after `=` holds one value
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

/** The item texts of a typed line (what {@link parseFilter} reads when it is given a string). */
export const splitItems = (text: string): string[] => items(text).map((item) => item.text)

const csv = (raw: string): string[] => raw.split(',').map((part) => part.trim()).filter((part) => part !== '')
const normalizeTag = (raw: string): string => raw.trim().replace(/^#+/, '').trim().toLowerCase()
const splitTags = (raw: string): string[] => csv(raw).map(normalizeTag).filter((tag) => tag !== '')
const sel = (values: string[]): Sel[] => values.map((value) => (value === 'none' ? { unset: true } : { unset: false, is: value }))

const pair = (one: string): [string, string] | undefined => {
  const at = one.trim().indexOf('=')
  if (at < 0) return undefined
  const text = one.trim()
  return [text.slice(0, at).trim(), text.slice(at + 1).trim()]
}

// ---- time -----------------------------------------------------------------------------------------------

const daysIn = (year: number, month: number): number => {
  if ([1, 3, 5, 7, 8, 10, 12].includes(month)) return 31
  if ([4, 6, 9, 11].includes(month)) return 30
  return year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0) ? 29 : 28
}
const epochDay = (year: number, month: number, day: number): number => Math.floor(Date.UTC(year, month - 1, day) / 86_400_000)

/** `2026-09-30T04:12:03Z` to epoch seconds, leniently (the way the tracker's own files are read). */
function parseStamp(raw: string): number | undefined {
  const m = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})Z$/.exec(raw)
  if (!m) return undefined
  const [year = 0, month = 0, day = 0, h = 0, mi = 0, s = 0] = m.slice(1).map(Number)
  if (month < 1 || month > 12 || day < 1 || day > 31 || h > 23 || mi > 59 || s > 60) return undefined
  return epochDay(year, month, day) * 86_400 + h * 3600 + mi * 60 + s
}

/** `2026-09-30` to epoch days, strictly: no day the calendar does not have. */
function parseDate(raw: string): number | undefined {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(raw)
  if (!m) return undefined
  const [year = 0, month = 0, day = 0] = m.slice(1).map(Number)
  if (month < 1 || month > 12 || day < 1 || day > daysIn(year, month)) return undefined
  return epochDay(year, month, day)
}

/** `2026-09-30T04:12:03Z` to epoch seconds, strictly (what a person types). */
function parseInstant(raw: string): number | undefined {
  const m = /^(\d{4}-\d{2}-\d{2})T(\d{2}):(\d{2}):(\d{2})Z$/.exec(raw)
  if (!m) return undefined
  const day = parseDate(m[1] ?? '')
  const [h = 0, mi = 0, s = 0] = m.slice(2).map(Number)
  if (day === undefined || h > 23 || mi > 59 || s > 59) return undefined
  return day * 86_400 + h * 3600 + mi * 60 + s
}

type Bad<T> = Result<T, BadFilter>
const bad = (error: BadFilter): { ok: false; error: BadFilter } => ({ ok: false, error })

/** One end: `…T…Z` is an instant, `YYYY-MM-DD` the reader's day, `YYYY-MM-DD HH:MM` the reader's minute. */
function instant(raw: string, end: boolean): Bad<{ at: number; wall: boolean }> {
  const text = raw.trim()
  const at = parseInstant(text)
  if (at !== undefined) return { ok: true, value: { at, wall: false } }
  const day = parseDate(text)
  if (day !== undefined) return { ok: true, value: { at: day * 86_400 + (end ? 86_399 : 0), wall: true } }
  const m = /^(\S+)\s+(.+)$/.exec(text)
  const minute = m ? parseInstant(`${m[1]}T${(m[2] ?? '').trimStart()}:00Z`) : undefined
  if (minute !== undefined) return { ok: true, value: { at: minute + (end ? 59 : 0), wall: true } }
  return bad({ code: 'not-a-time', value: text })
}

/** `--since`: open to the future from that time. */
function spanSince(raw: string): Bad<Span> {
  const from = instant(raw, false)
  return from.ok ? { ok: true, value: { from: from.value } } : from
}

/** `from~to` or `from..to` (either side may be empty), or one time standing for all it spans. */
function spanOf(raw: string): Bad<Span> {
  let cut = raw.indexOf('~')
  let width = 1
  if (cut < 0) {
    cut = raw.indexOf('..')
    width = 2
  }
  let span: Span
  if (cut < 0) {
    const from = instant(raw, false)
    const to = instant(raw, true)
    if (!from.ok) return from
    if (!to.ok) return to
    span = { from: from.value, to: to.value }
  } else {
    const a = raw.slice(0, cut).trim()
    const b = raw.slice(cut + width).trim()
    if (a === '' && b === '') return bad({ code: 'endless', value: raw })
    span = {}
    if (a !== '') {
      const from = instant(a, false)
      if (!from.ok) return from
      span.from = from.value
    }
    if (b !== '') {
      const to = instant(b, true)
      if (!to.ok) return to
      span.to = to.value
    }
  }
  const { from, to } = span
  // Mixed units cannot be compared without the zone, so only a gap wider than a day is refused.
  if (from && to && (from.wall === to.wall ? from.at > to.at : from.at > to.at + 86_400)) return bad({ code: 'backwards', value: raw })
  return { ok: true, value: span }
}

/** Splits a value on commas and reads each piece as a span: the pieces are "or". Nothing at all is refused. */
function spans(raw: string[], one: (piece: string) => Bad<Span>): Bad<Span[][]> {
  const out: Span[][] = []
  for (const value of raw) {
    const pieces = csv(value)
    if (pieces.length === 0) return bad({ code: 'no-time' })
    const any: Span[] = []
    for (const piece of pieces) {
      const read = one(piece)
      if (!read.ok) return read
      any.push(read.value)
    }
    out.push(any)
  }
  return { ok: true, value: out }
}

const spansHold = (all: Span[][], at: number | undefined, local?: (epoch: number) => number): boolean => {
  if (at === undefined) return all.length === 0
  const wall = local ? local(at) : at
  return all.every((any) =>
    any.some((span) => {
      const side = (end: { at: number; wall: boolean }): number => (end.wall ? wall : at)
      return (!span.from || side(span.from) >= span.from.at) && (!span.to || side(span.to) <= span.to.at)
    }),
  )
}

// ---- build ----------------------------------------------------------------------------------------------

const KINDS = ['issue', 'epic', 'milestone', 'backlog'] as const

/**
 * Reads filter text (one line, or the items already cut) into a {@link Filter}. A repeat of `status`,
 * `epic`, `milestone`, `parent` or `priority` is refused (use a comma), and so is a second `grep`, `type`
 * or `stale`. A text with `grep=` also lets backlog and archived rows in; a time question lets in
 * everything hidden, as the CLI does.
 */
export function parseFilter(input: string | readonly string[], flags: Flags = {}): Bad<Filter> {
  const texts = typeof input === 'string' ? splitItems(input) : input.map((one) => one.trim()).filter((one) => one !== '')
  // No prototype: a typed key such as `constructor` or `__proto__` is a key to refuse, not a member to find.
  const raw: Record<string, string[]> = Object.create(null)
  for (const text of texts) {
    const read = pair(text)
    if (read) (raw[read[0]] ??= []).push(read[1])
  }
  // A second value of a one-value key is counted over the items that are pairs, before any item is read
  // (moai's `single` runs before `desugar`).
  for (const key of single) {
    const values = raw[key] ?? []
    if (values.length >= 2) return bad({ code: 'twice', key, values })
  }
  // Then items are read in the order typed: the first one to be wrong is the one reported.
  for (const text of texts) {
    const read = pair(text)
    if (!read) return bad({ code: 'not-a-pair', value: text.trim() })
    const [key, value] = read
    if (!(KEYS as readonly string[]).includes(key)) return bad({ code: 'no-such-key', key })
    if (key === 'type' && !(KINDS as readonly string[]).includes(value) && value !== 'idea') return bad({ code: 'not-a-kind', value })
    if (key === 'stale' && !/^[+-]?\d+$/.test(value)) return bad({ code: 'not-days', value })
  }
  const get = (...keys: string[]): string[] => keys.flatMap((key) => raw[key] ?? [])
  const one = (key: Key): Bad<string[]> => {
    const values = raw[key] ?? []
    if (values.length >= 2) return bad({ code: 'twice', key, values })
    return { ok: true, value: values.length ? csv(values[0] ?? '') : [] }
  }
  const status = one('status')
  if (!status.ok) return status
  const done = spans(get('done'), spanOf)
  if (!done.ok) return done
  if (done.value.length && status.value.length && !status.value.includes('done')) {
    return bad({ code: 'done-outside', asked: status.value.join(',') })
  }
  const epic = one('epic')
  if (!epic.ok) return epic
  const milestone = one('milestone')
  if (!milestone.ok) return milestone
  const parent = one('parent')
  if (!parent.ok) return parent
  const priorityTexts = one('priority')
  if (!priorityTexts.ok) return priorityTexts
  const priority: number[] = []
  for (const text of priorityTexts.value) {
    const digits = text.startsWith('p') ? text.slice(1) : text
    const n = /^\+?\d+$/.test(digits) ? Number(digits) : NaN
    if (!(n >= 0 && n <= 3)) return bad({ code: 'not-a-priority', value: text })
    priority.push(n)
  }
  const since = spans(get('since'), spanSince)
  if (!since.ok) return since
  const updatedAt = spans(get('updated_at'), spanOf)
  if (!updatedAt.ok) return updatedAt
  const created = spans(get('created', 'created_at'), spanOf)
  if (!created.ok) return created
  const started = spans(get('started_at'), spanOf)
  if (!started.ok) return started
  const doneAt = spans(get('done_at'), spanOf)
  if (!doneAt.ok) return doneAt
  const kindText = raw.type?.[0]
  const kind = kindText === undefined ? undefined : kindText === 'idea' ? 'backlog' : (kindText as Filter['kind'])
  const grep = raw.grep?.[0]
  const stale = raw.stale?.[0] === undefined ? undefined : Number(raw.stale[0])
  const filter: Filter = {
    status: status.value,
    tags: get('tag').map(splitTags).filter((tags) => tags.length > 0),
    noTags: get('no-tag').flatMap(splitTags),
    epic: sel(epic.value),
    milestone: sel(milestone.value),
    parent: sel(parent.value),
    priority,
    // A repeated assignee is "or" as well: one row has one assignee, so "and" would be empty forever.
    assignee: sel(get('assignee').flatMap(csv)),
    kind,
    grep: grep?.toLowerCase(),
    grepIn: flags.grepIn ?? 'all',
    stale,
    updated: [...since.value, ...updatedAt.value],
    created: created.value,
    done: done.value,
    started: started.value,
    doneAt: doneAt.value,
    all: Boolean(flags.all || flags.archived),
    archived: Boolean(flags.archived || stale !== undefined || grep !== undefined),
    deferred: flags.deferred ? true : undefined,
    backlog: Boolean(flags.backlog || kind === 'backlog' || grep !== undefined || flags.deferred),
  }
  const asksTime = [filter.updated, filter.created, filter.done, filter.started, filter.doneAt].some((list) => list.length > 0)
  if (asksTime) filter.all = filter.archived = filter.backlog = true
  return { ok: true, value: filter }
}

/** English wording of a refusal, for places that have no catalog of their own (a tooltip, a log line). */
export function badFilterMessage(error: BadFilter): string {
  switch (error.code) {
    case 'twice':
      return `${error.key} is given twice (${error.values.join(', ')})${(once as readonly string[]).includes(error.key) ? ' - join the values with a comma' : ''}`
    case 'done-outside':
      return `done= looks at rows in done, but status=${error.asked} leaves it out`
    case 'endless':
      return `\`${error.value}\` has no end at all`
    case 'backwards':
      return `\`${error.value}\` ends before it begins`
    case 'not-a-time':
      return `\`${error.value}\` is not a time (YYYY-MM-DD, YYYY-MM-DD HH:MM or YYYY-MM-DDTHH:MM:SSZ)`
    case 'no-time':
      return 'a time is needed here'
    case 'not-a-pair':
      return `\`${error.value}\` is not item=value`
    case 'no-such-key':
      return `no filter item \`${error.key}\` - the items are ${KEYS.join(', ')}`
    case 'not-days':
      return `\`${error.value}\` is not a number of days`
    case 'not-a-priority':
      return `\`${error.value}\` is not a priority (0-3)`
    case 'not-a-kind':
      return `\`${error.value}\` is not a kind - the kinds are issue, epic, milestone, backlog`
  }
}

// ---- match ----------------------------------------------------------------------------------------------

/** `name (email)` split like moai's `Actor::parse`: the `(` that the last `)` closes, else the last `(`. */
function splitAssignee(raw: string): { name?: string; email?: string } {
  const text = raw.trim()
  if (text === '') return {}
  if (text.endsWith(')')) {
    const inner = text.slice(0, -1)
    const sane = (open: number): { name: string; email: string } | undefined => {
      const name = inner.slice(0, open).trim()
      const email = inner.slice(open + 1).trim()
      return name !== '' && !/[\r\n]/.test(name) && email.includes('@') && !/\s/.test(email) ? { name, email } : undefined
    }
    let depth = 0
    let open: number | undefined
    for (let at = inner.length - 1; at >= 0; at--) {
      const c = inner[at]
      if (c === ')') depth++
      else if (c === '(' && depth === 0) {
        open = at
        break
      } else if (c === '(') depth--
      else if (isSpace(c ?? '')) break
    }
    const found = (open === undefined ? undefined : sane(open)) ?? (inner.lastIndexOf('(') >= 0 ? sane(inner.lastIndexOf('(')) : undefined)
    if (found) return found
  }
  return { name: text }
}

const unassigned = (row: Row): boolean => row.assignee === undefined || row.assignee === null || String(row.assignee).trim() === ''

/** Whether the row is assigned to `want` (a name, an email or `name (email)`; one of the two is enough). */
function isAssignee(want: Sel, row: Row): boolean {
  if (want.unset) return unassigned(row)
  const { name, email } = splitAssignee(want.is)
  const mail = (e: string): boolean => typeof row.assignee_email === 'string' && row.assignee_email.toLowerCase() === e.toLowerCase()
  return (name !== undefined && row.assignee === name) || (email !== undefined && mail(email)) || mail(want.is)
}

const selHolds = (list: Sel[], value: string | undefined): boolean =>
  list.length === 0 || list.some((one) => (one.unset ? value === undefined : value === one.is))

/** Answers about a row that need the other rows: its column, epic, milestone, deferral, archive. */
interface Lens {
  column(row: Row): string
  epic(row: Row): string | undefined
  milestone(row: Row): string | undefined
  deferred(row: Row): boolean
  archived(row: Row): boolean
  since(row: Row): string
}

const str = (value: unknown): string | undefined => (typeof value === 'string' && value !== '' ? value : undefined)

/** Reads the answers from the rows' JSON fields; the context's hooks take their place when given. */
function lensOf(rows: readonly Row[], ctx: Context): Lens {
  const byId = new Map<string, Row>()
  for (const row of rows) byId.set(row.id, row)
  const column = (row: Row): string => str(row.derived_status) ?? str(row.status) ?? ''
  const epic = (row: Row): string | undefined => (ctx.epicOf ? ctx.epicOf(row) : (str(row.derived_epic) ?? undefined))
  // The index of the rows (nav.rs placing, report.rs `Shelf`), built on first use only; a row not among `rows`
  // asks by its id.
  let shelf: { index: IssueIndex; at: Map<Row, number> } | undefined
  const indexed = () => (shelf ??= { index: new IssueIndex(rows), at: new Map(rows.map((one, at) => [one, at])) })
  const milestone = (row: Row): string | undefined => {
    if (ctx.milestoneOf) return ctx.milestoneOf(row)
    // moai's `derived_milestone` wins when the row carries it (null or empty: none); 0.17 leaves it out under none.
    if (row.derived_milestone !== undefined) return str(row.derived_milestone)
    // The milestone the row stands on as moai reads it (`report::stood_at_line`): the epic beats the milestone
    // it wrote, an id child stands where its fold top stands (its own field is not read), and a row naming an
    // epic that is not there stands on none.
    const { index, at } = indexed()
    const i = at.get(row)
    if (i !== undefined) return index.stoodAtLine(i)
    const e = epic(row)
    const home = e === undefined ? undefined : byId.get(e)
    return home ? str(home.milestone) : str(row.milestone)
  }
  const own = (row: Row): boolean => str(row.deferred_at) !== undefined
  // The inherited deferral is the index's walk (report.rs `Shelf`): up the id parents, through each one's epic
  // and milestone, never into a closed row.
  const deferred = (row: Row): boolean => {
    if (ctx.deferredOf) return ctx.deferredOf(row)
    // moai 0.17 `shelved_by` (present only when the row is put off) wins (the index's shelf reads it too); its
    // absence leaves the walk below to decide, as on an older moai.
    if (str(row.shelved_by) !== undefined) return true
    if (own(row)) return true
    const { index, at } = indexed()
    const i = at.get(row)
    return (i === undefined ? index.deferredRoot(row.id) : index.shelvedAt(i)) !== undefined
  }
  const since = (row: Row): string => str(row.status_since) ?? str(row.updated_at) ?? ''
  const archived = (row: Row): boolean => {
    // moai 0.17 `archived: true` (by the CLI's own archive_days), or source.ts markArchived's tag where moai left
    // the key out (an older moai, or a row the clock archived after the archive was read).
    if (row.archived === true) return true
    if (!ctx.archiveDays || ctx.archiveDays <= 0 || !ctx.now || column(row) !== 'done') return false
    const now = parseStamp(ctx.now)
    const at = parseStamp(since(row))
    return now !== undefined && at !== undefined && Math.max(0, Math.floor((now - at) / 86_400)) >= ctx.archiveDays
  }
  return { column, epic, milestone, deferred, archived, since }
}

const lowerHas = (value: unknown, q: string): boolean => typeof value === 'string' && value.toLowerCase().includes(q)

/** Whether `q` (case-folded) is in the parts of the row the scope sees; notes are looked in when given. */
export function hits(row: Row, q: string, scope: GrepScope, ctx: Context = {}): boolean {
  const needle = q.toLowerCase()
  return (
    (sees(scope, 'id') && lowerHas(row.id, needle)) ||
    (sees(scope, 'title') && lowerHas(row.title, needle)) ||
    (sees(scope, 'tag') && Array.isArray(row.tags) && row.tags.some((tag) => lowerHas(tag, needle))) ||
    (sees(scope, 'body') && lowerHas(row.body, needle)) ||
    (sees(scope, 'note') && Boolean(ctx.notes && Object.prototype.hasOwnProperty.call(ctx.notes, row.id) && ctx.notes[row.id]?.some((note) => note.toLowerCase().includes(needle))))
  )
}

/** Rows whose scope-seen parts hold `q`. Plain text search: unlike {@link applyFilter} it hides nothing. */
export function search<T extends Row>(rows: readonly T[], q: string, scope: GrepScope = 'all', ctx: Context = {}): T[] {
  return q.trim() === '' ? [...rows] : rows.filter((row) => hits(row, q, scope, ctx))
}

/** The lines of the notes that hold `q`, case-folded: why a row was found by its notes. */
export function noteLines(texts: readonly string[], q: string): string[] {
  if (q.trim() === '') return []
  const needle = q.toLowerCase()
  return texts.flatMap((note) => note.split(/\r?\n/)).filter((line) => line.toLowerCase().includes(needle))
}

const kindName = (row: Row): 'issue' | 'epic' | 'milestone' | 'backlog' => {
  if (isBacklog(row)) return 'backlog'
  if (isGroup(row)) return row.kind === 'milestone' ? 'milestone' : 'epic'
  return 'issue'
}

/** One row against the filter, given the answers that need the other rows. */
function holds(row: Row, filter: Filter, lens: Lens, ctx: Context): boolean {
  const q = filter.grep
  // Hidden by default: deferred and done rows, the archive, backlog rows; each is opened by its own switch.
  if (!filter.archived && lens.archived(row)) return false
  if (filter.deferred !== undefined && lens.deferred(row) !== filter.deferred) return false
  if (filter.deferred === undefined && !filter.all && lens.deferred(row)) return false
  if (!filter.all && filter.status.length === 0 && lens.column(row) === 'done') return false
  if (isBacklog(row) && !filter.backlog) return false
  if (filter.status.length && !filter.status.includes(lens.column(row))) return false
  const tags = Array.isArray(row.tags) ? (row.tags as string[]) : []
  if (!filter.tags.every((any) => any.some((tag) => tags.includes(tag)))) return false
  if (filter.noTags.some((tag) => tags.includes(tag))) return false
  if (!selHolds(filter.epic, lens.epic(row))) return false
  if (filter.milestone.length && !selHolds(filter.milestone, lens.milestone(row))) return false
  if (!selHolds(filter.parent, parentOf(row.id))) return false
  if (filter.priority.length && !filter.priority.includes(priorityOf(row))) return false
  if (filter.assignee.length && !filter.assignee.some((want) => isAssignee(want, row))) return false
  if (filter.kind && kindName(row) !== filter.kind) return false
  if (filter.stale !== undefined) {
    const now = ctx.now === undefined ? undefined : parseStamp(ctx.now)
    const at = parseStamp(lens.since(row))
    const days = now === undefined || at === undefined ? undefined : Math.max(0, Math.floor((now - at) / 86_400))
    if (days === undefined || days < filter.stale) return false
  }
  const within = (list: Span[][], at: number | undefined): boolean => spansHold(list, at, ctx.local)
  const stamp = (value: unknown): number | undefined => (typeof value === 'string' ? parseStamp(value) : undefined)
  if (filter.updated.length && !within(filter.updated, stamp(row.updated_at))) return false
  if (filter.created.length && !within(filter.created, stamp(row.created_at))) return false
  // `done` is the time it got to the column it stands in now; only a row standing in done has one.
  if (filter.done.length && !within(filter.done, lens.column(row) === 'done' ? stamp(lens.since(row)) : undefined)) return false
  if (filter.started.length && !within(filter.started, stamp(row.started_at))) return false
  if (filter.doneAt.length && !within(filter.doneAt, stamp(row.done_at))) return false
  if (q !== undefined && !hits(row, q, filter.grepIn, ctx)) return false
  return true
}

/** Whether one row passes the filter. `rows` are all the rows (the epic chain is read from them). */
export function matches(row: Row, filter: Filter, rows: readonly Row[], ctx: Context = {}): boolean {
  return holds(row, filter, lensOf(rows, ctx), ctx)
}

/** The rows that pass the filter, in the order given. */
export function applyFilter<T extends Row>(rows: readonly T[], filter: Filter, ctx: Context = {}): T[] {
  const lens = lensOf(rows, ctx)
  return rows.filter((row) => holds(row, filter, lens, ctx))
}

/**
 * Why the default list hides a row, or `undefined` when it shows it: the one word that would open it, as
 * moai's `hidden_by`. A row no single word opens (a closed backlog row, an archived one) is `unopenable`,
 * which the CLI's tail does not count. A deferred backlog row is `deferred` (that switch opens backlog too);
 * a deferred row in done is `done`.
 */
export function hiddenBy(row: Row, filter: Filter, rows: readonly Row[], ctx: Context = {}): Hidden {
  return hiding(filter, rows, ctx)(row)
}

/** Why the default list hides a row ({@link hiddenBy}). */
export type Hidden = 'done' | 'backlog' | 'deferred' | 'archived' | 'unopenable' | undefined

/** {@link hiddenBy} for many rows of one list: what needs the other rows is read once. */
export function hiding(filter: Filter, rows: readonly Row[], ctx: Context = {}): (row: Row) => Hidden {
  const lens = lensOf(rows, ctx)
  return (row) => hiddenWith(row, filter, lens)
}

function hiddenWith(row: Row, filter: Filter, lens: Lens): Hidden {
  const backlog = isBacklog(row) && !filter.backlog
  if (!filter.archived && lens.archived(row)) return backlog ? 'unopenable' : 'archived'
  const deferred = filter.deferred === undefined && !filter.all && lens.deferred(row)
  const done = !filter.all && filter.status.length === 0 && lens.column(row) === 'done'
  if (!backlog && !deferred && !done) return undefined
  if (!deferred && !done) return 'backlog'
  if (!done) return 'deferred'
  if (!backlog) return 'done'
  return 'unopenable'
}

// ---- sort -----------------------------------------------------------------------------------------------

/** The fields a list may be ordered by. */
export type SortKey = 'priority' | 'created' | 'updated' | 'status' | 'assignee' | 'title' | 'id'
/** The sort fields in the order moai names them (also the order the sort menu lists them). */
export const SORT_KEYS: readonly SortKey[] = ['priority', 'created', 'updated', 'status', 'assignee', 'title', 'id']
export type Dir = 'asc' | 'desc'
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
export type BadSort =
  | { code: 'empty' }
  | { code: 'unknown'; word: string }
  | { code: 'direction'; word: string; dir: string }
  | { code: 'repeated'; key: SortKey }

/** The direction a field has when none is written: newest first for created and updated, else ascending. */
export const defaultDir = (key: SortKey): Dir => (key === 'created' || key === 'updated' ? 'desc' : 'asc')
/** A field in its own direction. */
export const fieldOf = (key: SortKey): SortField => ({ key, dir: defaultDir(key) })
/** The order moai shows when none is chosen: priority, then id. */
export const DEFAULT_SORT: Sort = { fields: [fieldOf('priority')], reversed: false }

/** Writes fields as `parseSort` reads them: the key alone in its own direction, else `key:asc|desc`. */
export const formatSort = (fields: readonly SortField[]): string =>
  fields.map((field) => (field.dir === defaultDir(field.key) ? field.key : `${field.key}:${field.dir}`)).join(',')

/** The same order: the same fields in the same directions, and the same reverse switch. */
export const sameSort = (a: Sort, b: Sort): boolean => a.reversed === b.reversed && formatSort(a.fields) === formatSort(b.fields)

/** Reads `priority,updated:desc` into fields. A key may stand once; an empty slot or unknown word is refused. */
export function parseSort(raw: string): Result<SortField[], BadSort> {
  const fields: SortField[] = []
  for (const part of raw.split(',').map((one) => one.trim())) {
    if (part === '') return { ok: false, error: { code: 'empty' } }
    const colon = part.indexOf(':')
    const word = (colon < 0 ? part : part.slice(0, colon)).trim()
    const dirText = colon < 0 ? undefined : part.slice(colon + 1).trim()
    if (word === '') return { ok: false, error: { code: 'empty' } }
    const key = SORT_KEYS.find((one) => one === word)
    if (!key) return { ok: false, error: { code: 'unknown', word } }
    if (dirText !== undefined && dirText !== 'asc' && dirText !== 'desc') return { ok: false, error: { code: 'direction', word, dir: dirText } }
    if (fields.some((field) => field.key === key)) return { ok: false, error: { code: 'repeated', key } }
    fields.push({ key, dir: dirText ?? defaultDir(key) })
  }
  return { ok: true, value: fields }
}

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
const compareCaseless = (a: string, b: string): number => compareCodePoints(a.toLowerCase(), b.toLowerCase())

/** What the screen shows for an assignee, as moai's `label`: `name (email)`, the name or the email. */
function shownName(row: Row, ctx: Context): string {
  const name = String(row.assignee ?? '')
  const email = typeof row.assignee_email === 'string' && row.assignee_email.trim() !== '' ? row.assignee_email.trim() : undefined
  if (email === undefined) return name
  if (name.trim() === '') return email
  const naming = ctx.naming ?? 'full'
  return naming === 'name' ? name : naming === 'email' ? email : `${name} (${email})`
}

/**
 * Compares two rows in an order, as moai's `order_by`: fields front to back, each in its own direction; if
 * all tie, id decides when the last field is created or updated, else priority then id. `reversed` turns
 * the whole result, ties included. The column of a group is its `derived_status`.
 */
export function orderBy(a: Row, b: Row, fields: readonly SortField[], reversed = false, ctx: Context = {}): number {
  const statuses = ctx.statuses ?? DEFAULT_STATUSES
  const rank = (row: Row): number => {
    const at = statuses.indexOf(str(row.derived_status) ?? str(row.status) ?? '')
    return at < 0 ? statuses.length : at
  }
  const stamp = (row: Row, key: 'created_at' | 'updated_at'): string => String(row[key] ?? '')
  const rising = (key: SortKey): number => {
    switch (key) {
      case 'priority':
        return priorityOf(a) - priorityOf(b)
      case 'created':
        return compareCodePoints(stamp(a, 'created_at'), stamp(b, 'created_at'))
      case 'updated':
        return compareCodePoints(stamp(a, 'updated_at'), stamp(b, 'updated_at'))
      case 'status':
        return rank(a) - rank(b)
      case 'assignee':
        // A row without an assignee goes last when ascending (first when descending).
        if (unassigned(a) || unassigned(b)) return Number(unassigned(a)) - Number(unassigned(b))
        return compareCaseless(shownName(a, ctx), shownName(b, ctx))
      case 'title':
        return compareCaseless(String(a.title ?? ''), String(b.title ?? ''))
      case 'id':
        return compareCodePoints(a.id, b.id)
    }
  }
  let order = 0
  for (const field of fields) {
    const d = rising(field.key)
    if (d !== 0) {
      order = field.dir === 'asc' ? d : -d
      break
    }
  }
  if (order === 0) {
    const last = fields[fields.length - 1]?.key
    order = last === 'created' || last === 'updated' ? compareCodePoints(a.id, b.id) : displayOrder(a, b)
  }
  return reversed ? -order : order
}

/** The rows in the order (a new array; the sort is stable). */
export function sortRows<T extends Row>(rows: readonly T[], sort: Sort | readonly SortField[], ctx: Context = {}): T[] {
  const { fields, reversed } = Array.isArray(sort) ? { fields: sort as SortField[], reversed: false } : (sort as Sort)
  return [...rows].sort((a, b) => orderBy(a, b, fields, reversed, ctx))
}

// ---- sort edits and badge --------------------------------------------------------------------------------

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

/** Filter keys that start with `prefix` (all of them when empty), in moai's order. */
export const keyHints = (prefix = ''): Key[] => KEYS.filter((key) => key.startsWith(prefix.trim().toLowerCase()))

const tally = (values: Iterable<string>): Map<string, number> => {
  const counts = new Map<string, number>()
  for (const value of values) counts.set(value, (counts.get(value) ?? 0) + 1)
  return counts
}
const byCount = (a: Hint, b: Hint): number => b.count - a.count || compareCodePoints(a.value, b.value)

/**
 * Value candidates for `key` from the rows: tags (`tag`, `no-tag`), assignee names, epics and milestones
 * (id with the title as label, plus `none`), parents, columns (in the config's order), priorities, kinds.
 * Narrowed by `prefix` against the value and its label, case-folded (a value that begins with it first); most-carried first.
 */
export function valueHints(rows: readonly Row[], key: string, prefix = '', ctx: Context = {}): Hint[] {
  const needle = prefix.trim().toLowerCase()
  const lens = lensOf(rows, ctx)
  let hints: Hint[] = []
  const none = (count: number): Hint[] => (count > 0 ? [{ value: 'none', count }] : [])
  const named = (kind: 'epic' | 'milestone', of: (row: Row) => string | undefined): Hint[] => {
    const counts = tally(rows.flatMap((row) => of(row) ?? []))
    const owners = rows.filter((row) => row.kind === kind)
    const listed = owners.map((owner): Hint => ({ value: owner.id, count: counts.get(owner.id) ?? 0, label: owner.title }))
    for (const [id, count] of counts) if (!owners.some((owner) => owner.id === id)) listed.push({ value: id, count })
    return [...listed.sort(byCount), ...none(rows.filter((row) => isGroup(row) === false && of(row) === undefined).length)]
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
      hints = named('epic', lens.epic)
      break
    case 'milestone':
      hints = named('milestone', lens.milestone)
      break
    case 'parent': {
      const counts = tally(rows.flatMap((row) => parentOf(row.id) ?? []))
      hints = [...counts].map(([value, count]) => ({ value, count, label: rows.find((row) => row.id === value)?.title })).sort(byCount)
      hints.push(...none(rows.filter((row) => parentOf(row.id) === undefined).length))
      break
    }
    case 'status': {
      const statuses = ctx.statuses ?? DEFAULT_STATUSES
      const rank = (value: string): number => (statuses.indexOf(value) < 0 ? statuses.length : statuses.indexOf(value))
      hints = [...tally(rows.map(lens.column).filter((value) => value !== ''))]
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

const TIME_KEYS: readonly string[] = ['since', 'created', 'done', 'created_at', 'updated_at', 'started_at', 'done_at']

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
export function hintsAt(rows: readonly Row[], text: string, cursor: number, ctx: Context = {}): { slot: Slot; hints: Hint[] } {
  const slot = slotAt(text, cursor)
  const hints =
    slot.key === undefined
      ? keyHints(slot.partial).map((value): Hint => ({ value, count: 0 }))
      : valueHints(rows, slot.key, slot.partial, ctx)
  return { slot, hints }
}
