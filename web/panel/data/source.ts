/**
 * Where the page's data comes from: one function per `moai web` endpoint, each handing out plain values or the
 * {@link Outcome} that stopped it. Nothing above this module knows which endpoint answered.
 *
 * - The index rows come from `/api/rows` (`show --all`: active rows, done and deferred included, archive left out)
 *   and `/api/backlog` (`backlog show --all`, the backlog the same way), read side by side ({@link loadAllRows}).
 *   A failure of either is the load's.
 * - A partial answer (Moai's non-zero exit with its whole JSON still printed; see client.ts) is read like a
 *   whole one: the rows it could read, the wiki pages it did not skip. What moai said beside it rides as `notes`.
 * - The milestone and epic counts come from `/api/status` ({@link loadTallies}).
 * - The archive is read when asked for, once each list with `archived=1` ({@link loadArchived}): `show --archived`
 *   prints the active rows and the archive together, and marks the archive's own `archived: true`.
 * - The wiki listing and pages are handed out as Moai printed them ({@link loadWikiListing},
 *   {@link loadWikiPage}); the wiki view parses them.
 * - What the page boots on — the language, the columns and their glyphs — is `/api/boot` ({@link loadBoot}), which
 *   the server answers itself, without the envelope.
 */
import { PRIORITY, answerOf, outcomeOf, type Client, type Endpoint, type Fetch, type Outcome, type RequestOptions } from './client.js'
import type { Row } from './index.js'

export const ENDPOINT = {
  boot: '/api/boot',
  stamp: '/api/stamp',
  status: '/api/status',
  rows: '/api/rows',
  backlog: '/api/backlog',
  issue: (id: string): Endpoint => ({ path: `/api/issue/${encodeURIComponent(id)}` }),
  wiki: '/api/wiki',
  wikiPage: (slug: string): Endpoint => ({ path: `/api/wiki/${slug.split('/').map(encodeURIComponent).join('/')}` }),
} as const

export interface Rows {
  rows: Row[]
  /** What moai said on stderr beside the answers (a line it could not read). */
  notes?: string[]
}

export type Loaded<T> = { ok: true; value: T } | { ok: false; outcome: Outcome }

const isRecord = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null
const items = (list: unknown): Row[] =>
  (Array.isArray(list) ? list : []).filter((item): item is Row => isRecord(item) && typeof item.id === 'string')
const notesOf = (...outcomes: Outcome[]): string[] =>
  outcomes.flatMap((outcome) => ('notes' in outcome && outcome.notes ? outcome.notes : []))

/**
 * `rows`, then the backlog's rows marked kind `backlog` whatever they say. A backlog row whose id `rows` already
 * holds is left out; twins inside either list are all kept for the index.
 */
function withBacklog(rows: Row[], backlog: unknown): Row[] {
  const listed = new Set(rows.map((row) => row.id))
  return [...rows, ...items(backlog).filter((row) => !listed.has(row.id)).map((row) => ({ ...row, kind: 'backlog' }))]
}

/**
 * `show --all` and `backlog show --all` print a JSON array of rows; backlog rows are kind `backlog` whatever they
 * say. Twins (lines a merge left sharing one id) are not thinned out here: the index lets the later one win, as
 * moai does, and `show` prints them together, in file order when their priority ties.
 */
export function rowsFromLists(all: unknown, backlog: unknown): Loaded<Rows> {
  if (!Array.isArray(all) || !Array.isArray(backlog)) return { ok: false, outcome: { kind: 'unreadable' } }
  return { ok: true, value: { rows: withBacklog(items(all), backlog) } }
}

/** Both lists side by side, with `query` on each; the first that gave no answer is the load's failure. */
async function bothLists(client: Client, query: Record<string, string>, options: RequestOptions): Promise<Loaded<Rows>> {
  const [all, backlog] = await Promise.all([
    client.request({ path: ENDPOINT.rows, query }, options),
    client.request({ path: ENDPOINT.backlog, query }, options),
  ])
  for (const outcome of [all, backlog]) if (answerOf(outcome) === undefined) return { ok: false, outcome }
  const loaded = rowsFromLists(answerOf(all), answerOf(backlog))
  const notes = notesOf(all, backlog)
  if (loaded.ok && notes.length) loaded.value.notes = notes
  return loaded
}

/** The index rows: `/api/rows` and `/api/backlog` read at once. */
export function loadAllRows(client: Client, priority: number = PRIORITY.list, epoch?: number): Promise<Loaded<Rows>> {
  return bothLists(client, {}, { priority, epoch })
}

/**
 * `[done, total]` per milestone and epic id from `status --json` (`milestones[]` and `epics[]`). These count the
 * rows the clock has archived too, which `show --all` omits; hand the map to `new IssueIndex(rows, tallies)` so
 * the board's n/n equals `moai status`. Entries without a string id and numeric `done` and `total` are skipped.
 */
export function talliesFrom(output: unknown): Map<string, [number, number]> {
  const tallies = new Map<string, [number, number]>()
  if (!isRecord(output)) return tallies
  for (const list of [output.milestones, output.epics]) {
    if (!Array.isArray(list)) continue
    for (const entry of list) {
      if (!isRecord(entry) || typeof entry.id !== 'string' || entry.id === '') continue
      const { done, total } = entry
      if (typeof done !== 'number' || typeof total !== 'number' || !Number.isFinite(done) || !Number.isFinite(total)) continue
      tallies.set(entry.id, [done, total])
    }
  }
  return tallies
}

/** Two {@link talliesFrom} maps that give every milestone and epic the same `[done, total]`. */
export function sameTallies(a: ReadonlyMap<string, readonly [number, number]>, b: ReadonlyMap<string, readonly [number, number]> | undefined): boolean {
  if (!b || a.size !== b.size) return false
  for (const [id, [done, total]] of a) {
    const other = b.get(id)
    if (!other || other[0] !== done || other[1] !== total) return false
  }
  return true
}

export async function loadTallies(client: Client, priority: number = PRIORITY.list, epoch?: number): Promise<Loaded<Map<string, [number, number]>>> {
  const outcome = await client.request({ path: ENDPOINT.status }, { priority, epoch })
  const output = answerOf(outcome)
  return output === undefined ? { ok: false, outcome } : { ok: true, value: talliesFrom(output) }
}

/**
 * `rows` with `archived: true` on those whose id is not in `active`. moai prints the same key itself on the
 * `--archived` rows `show --all` hides as the archive: such a row is passed through as moai printed it, and one moai
 * left unmarked is still tagged by id — the clock may have archived it after the archive was read (withArchive).
 */
export function markArchived(rows: readonly Row[], active: ReadonlySet<string>): Row[] {
  return rows.map((row) => (row.archived === true || active.has(row.id) ? row : { ...row, archived: true }))
}

/**
 * The listed rows, then the archive's rows they do not hold (only while archived rows are shown), tagged
 * `archived: true` against the rows held now: a row the clock archived after the archive was read left `show --all`
 * and is in the archive's answer untagged (it was active then), and it stays in the list as an archived row.
 */
export function withArchive(listed: Pick<Rows, 'rows'>, archive: readonly Row[] | null): Row[] {
  const { rows } = listed
  if (!archive) return rows
  const held = new Set(rows.map((row) => row.id))
  const away = markArchived(archive, held).filter((row) => !held.has(row.id))
  return away.length ? [...rows, ...away] : rows
}

/**
 * Every row the archive holds, with the active ones among them: both lists once with `archived=1`. The caller keeps
 * whichever it needs ({@link withArchive}).
 */
export async function loadArchived(client: Client, options: { priority?: number; epoch?: number } = {}): Promise<Loaded<Row[]>> {
  const loaded = await bothLists(client, { archived: '1' }, { priority: options.priority ?? PRIORITY.background, epoch: options.epoch })
  return loaded.ok ? { ok: true, value: loaded.value.rows } : loaded
}

/** `moai wiki ls` as printed (a skipped page's partial answer included); the wiki view parses it. */
export async function loadWikiListing(client: Client): Promise<Loaded<unknown>> {
  const outcome = await client.request({ path: ENDPOINT.wiki }, { priority: PRIORITY.wiki })
  const output = answerOf(outcome)
  return output === undefined ? { ok: false, outcome } : { ok: true, value: output }
}

/** `moai wiki show <slug>` as printed; the wiki view parses it. */
export async function loadWikiPage(client: Client, slug: string): Promise<Loaded<unknown>> {
  const outcome = await client.request(ENDPOINT.wikiPage(slug), { priority: PRIORITY.wiki })
  const output = answerOf(outcome)
  return output === undefined ? { ok: false, outcome } : { ok: true, value: output }
}

/** One row with its body, children and journal (`moai show <id>`); `epoch` as client.ts RequestOptions says. */
export async function loadDetail(client: Client, id: string, options: Pick<RequestOptions, 'epoch'> = {}): Promise<Loaded<Row>> {
  const outcome = await client.request(ENDPOINT.issue(id), { priority: PRIORITY.detail, epoch: options.epoch })
  if (outcome.kind !== 'passed') return { ok: false, outcome }
  return isRecord(outcome.output) && typeof outcome.output.id === 'string' ? { ok: true, value: outcome.output as Row } : { ok: false, outcome: { kind: 'unreadable' } }
}

/** A board column only the board draws — not one of the configured statuses (src/web/boot.rs Column). */
export interface BootColumn {
  name: string
  glyph: string
}

/** What `/api/boot` gives (src/web/boot.rs Boot). */
export interface Boot {
  lang: string
  version: string
  project: string
  root: string
  statuses: string[]
  naming: string
  /** Per configured status; a status not in it draws {@link Boot.unknown_glyph}. */
  glyphs: Record<string, string>
  unknown_glyph: string
  columns: { backlog: BootColumn; shelved: BootColumn }
  filter_keys: string[]
  sort_fields: { name: string; dir: 'asc' | 'desc' }[]
}

const strings = (value: unknown): value is string[] => Array.isArray(value) && value.every((item) => typeof item === 'string')
const column = (value: unknown): value is BootColumn => isRecord(value) && typeof value.name === 'string' && typeof value.glyph === 'string'

/** The boot answer read, or null when it is not that shape. */
export function bootFrom(value: unknown): Boot | null {
  if (!isRecord(value)) return null
  const { lang, version, project, root, statuses, naming, glyphs, unknown_glyph, columns, filter_keys, sort_fields } = value
  if (typeof lang !== 'string' || typeof version !== 'string' || typeof project !== 'string' || typeof root !== 'string') return null
  if (!strings(statuses) || typeof naming !== 'string' || typeof unknown_glyph !== 'string' || !strings(filter_keys)) return null
  if (!isRecord(glyphs) || !Object.values(glyphs).every((glyph) => typeof glyph === 'string')) return null
  if (!isRecord(columns) || !column(columns.backlog) || !column(columns.shelved)) return null
  if (!Array.isArray(sort_fields) || !sort_fields.every((field) => isRecord(field) && typeof field.name === 'string' && (field.dir === 'asc' || field.dir === 'desc'))) return null
  return value as unknown as Boot
}

/** `/api/boot`, fetched outside the queue: nothing else can be read before it. */
export async function loadBoot(fetcher: Fetch): Promise<Loaded<Boot>> {
  let status: number
  let body: string
  try {
    const answer = await fetcher(ENDPOINT.boot, { signal: new AbortController().signal, headers: { Accept: 'application/json' } })
    status = answer.status
    body = await answer.text()
  } catch {
    return { ok: false, outcome: { kind: 'not-started' } }
  }
  if (status !== 200) return { ok: false, outcome: outcomeOf(status, body) }
  let parsed: unknown
  try {
    parsed = JSON.parse(body)
  } catch {
    return { ok: false, outcome: { kind: 'unreadable' } }
  }
  const boot = bootFrom(parsed)
  return boot ? { ok: true, value: boot } : { ok: false, outcome: { kind: 'unreadable' } }
}
