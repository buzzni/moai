/**
 * Where the page's data comes from: one function per `moai web` endpoint, each handing out plain values or the
 * {@link Outcome} that stopped it. Nothing above this module knows which endpoint answered.
 *
 * - The index rows come from `/api/rows` (`show --all`: active rows, done and deferred included, archive left out)
 *   and `/api/backlog` (`backlog show --all`, the backlog the same way), read side by side ({@link loadAllRows}).
 *   A failure of either is the load's.
 * - A partial answer (Moai's non-zero exit with its whole JSON still printed; see client.ts) is read like a
 *   whole one: the rows it could read, the wiki pages it did not skip. What moai said beside it rides as `notes`.
 * - Where each row stands, whether it opens, its `n/n` and the order are `/api/tree` (`tui --json --deep`,
 *   {@link loadTree}); the order the page asks for goes with it.
 * - What a filter or a search finds is moai's answer too: both lists again, with `fields=id` — `/api/rows` alone for
 *   a `type=` filter, which `backlog show` refuses ({@link loadMatches}).
 * - The milestone and epic counts per column come from `/api/status` ({@link loadGroups}).
 * - The archive is read when asked for, once each list with `archived=1` ({@link loadArchived}): `show --archived`
 *   prints the active rows and the archive together, and marks the archive's own `archived: true`.
 * - The wiki listing and pages are handed out as Moai printed them ({@link loadWikiListing},
 *   {@link loadWikiPage}); the wiki view parses them.
 * - What the page boots on — the language, the columns and their glyphs — is `/api/boot` ({@link loadBoot}), which
 *   the server answers itself, without the envelope.
 * - The registered projects the header lists are `/api/projects` (`project ls --json`, {@link loadProjects}).
 */
import { latestOf, userOf, type Latest } from './about.js'
import { PRIORITY, answerOf, outcomeOf, type Client, type Endpoint, type Fetch, type Outcome, type RequestOptions } from './client.js'
import type { Row, TreeRow } from './index.js'
import { projectsFrom, type Project } from './projects.js'
import { splitItems } from './query.js'

export const ENDPOINT = {
  boot: '/api/boot',
  stamp: '/api/stamp',
  status: '/api/status',
  rows: '/api/rows',
  backlog: '/api/backlog',
  /** `worktree` (the view's `SPC v w`): a row only another git worktree holds opens too (`show --worktree`, moai-m4ez.yb1). */
  issue: (id: string, worktree = false): Endpoint => ({ path: `/api/issue/${encodeURIComponent(id)}`, ...(worktree ? { query: { worktree: '1' } } : {}) }),
  tree: '/api/tree',
  projects: '/api/projects',
  wiki: '/api/wiki',
  wikiPage: (slug: string): Endpoint => ({ path: `/api/wiki/${slug.split('/').map(encodeURIComponent).join('/')}` }),
  wikiSearch: (query: string): Endpoint => ({ path: '/api/wiki-search', query: { q: query } }),
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
 * say. Twins (lines a merge left sharing one id) are not thinned out here: `show` prints each, and the index gives
 * each tree line its own (data/index.ts), the folder being the line moai opens.
 */
export function rowsFromLists(all: unknown, backlog: unknown): Loaded<Rows> {
  if (!Array.isArray(all) || !Array.isArray(backlog)) return { ok: false, outcome: { kind: 'unreadable' } }
  return { ok: true, value: { rows: withBacklog(items(all), backlog) } }
}

/**
 * Both lists side by side, with `query` on each and `rowsOnly` on `/api/rows` alone; the first that gave no answer
 * is the load's failure.
 */
async function bothLists(client: Client, query: Record<string, string>, options: RequestOptions, rowsOnly: Record<string, string> = {}): Promise<Loaded<Rows>> {
  const [all, backlog] = await Promise.all([
    client.request({ path: ENDPOINT.rows, query: { ...query, ...rowsOnly } }, options),
    client.request({ path: ENDPOINT.backlog, query }, options),
  ])
  for (const outcome of [all, backlog]) if (answerOf(outcome) === undefined) return { ok: false, outcome }
  const loaded = rowsFromLists(answerOf(all), answerOf(backlog))
  const notes = notesOf(all, backlog)
  if (loaded.ok && notes.length) loaded.value.notes = notes
  return loaded
}

/**
 * The index rows: `/api/rows` and `/api/backlog` read at once. `worktree` (the view's `SPC v w`, on by default) lays
 * the other git worktrees' rows over the list: `/api/rows?worktree=1`, which the server runs as `show --all
 * --worktree`. The backlog has no such overlay.
 */
export function loadAllRows(client: Client, priority: number = PRIORITY.list, epoch?: number, worktree = false): Promise<Loaded<Rows>> {
  return bothLists(client, {}, { priority, epoch }, worktree ? { worktree: '1' } : {})
}

/**
 * The counts per column of each milestone and epic `status --json` prints (`milestones[]` and `epics[]` `counts`).
 * These count the archive too, which `show --all` leaves out. Entries without a string id and a counts object are skipped.
 */
export function groupsFrom(output: unknown): Map<string, Record<string, number>> {
  const groups = new Map<string, Record<string, number>>()
  if (!isRecord(output)) return groups
  for (const list of [output.milestones, output.epics]) {
    if (!Array.isArray(list)) continue
    for (const entry of list) {
      if (!isRecord(entry) || typeof entry.id !== 'string' || entry.id === '' || !isRecord(entry.counts)) continue
      const counts: Record<string, number> = {}
      for (const [status, count] of Object.entries(entry.counts)) if (typeof count === 'number' && Number.isFinite(count)) counts[status] = count
      groups.set(entry.id, counts)
    }
  }
  return groups
}

/** Two {@link groupsFrom} maps that give every milestone and epic the same counts. */
export function sameGroups(a: ReadonlyMap<string, Readonly<Record<string, number>>>, b: ReadonlyMap<string, Readonly<Record<string, number>>> | undefined): boolean {
  if (!b || a.size !== b.size) return false
  for (const [id, counts] of a) {
    const other = b.get(id)
    if (!other || JSON.stringify(Object.entries(other)) !== JSON.stringify(Object.entries(counts))) return false
  }
  return true
}

/**
 * `worktree` (the view's `SPC v w`) counts over the same overlay the tree is read with (`status --worktree`,
 * moai-m4ez.u6a): the detail roll-up's bar and legend come from here and its n/n text from the tree, so a member only a
 * sibling worktree holds has to be in both.
 */
export async function loadGroups(client: Client, priority: number = PRIORITY.list, epoch?: number, worktree = false): Promise<Loaded<Map<string, Record<string, number>>>> {
  const outcome = await client.request({ path: ENDPOINT.status, ...(worktree ? { query: { worktree: '1' } } : {}) }, { priority, epoch })
  const output = answerOf(outcome)
  return output === undefined ? { ok: false, outcome } : { ok: true, value: groupsFrom(output) }
}

/**
 * `/api/tree` with the order the page asks for (query.ts sortQuery): every row, where it stands, in moai's order.
 * `worktree` (the view's `SPC v w`) places the rows only another git worktree holds the way the list reads them
 * (`tui --json --deep --worktree`, moai-m4ez.gcp): the same overlay as {@link loadAllRows}, so no row of the list is
 * missing from the tree. `archived` (the view's `SPC v o`) asks for the archive's lines too: without it the server
 * leaves them out, and they were 95% of a 3 000-row tracker's 689 KB answer, read again on every write
 * (moai-m4ez.4zq). The page reads the tree again when `SPC v o` turns, so the lines are there when the archive is drawn.
 * `zone` (data/zone.ts zoneNow) is the zone the page draws in: moai judges a row late (`gantt.overdue`) in the zone it is
 * told, not the server machine's, so the count agrees with the today line drawn beside it (moai-vj8w.7sh). A zone that
 * machine cannot read (an offset, an Intl name its tzdb lacks) is judged in the server's own zone instead.
 */
export async function loadTree(client: Client, query: Record<string, string> = {}, priority: number = PRIORITY.list, epoch?: number, worktree = false, archived = false, zone?: string): Promise<Loaded<TreeRow[]>> {
  const asked = { ...query, ...(worktree ? { worktree: '1' } : {}), ...(archived ? { archived: '1' } : {}), ...(zone ? { tz: zone } : {}) }
  const outcome = await client.request({ path: ENDPOINT.tree, query: asked }, { priority, epoch })
  const output = answerOf(outcome)
  if (output === undefined) return { ok: false, outcome }
  if (!Array.isArray(output)) return { ok: false, outcome: { kind: 'unreadable' } }
  return { ok: true, value: output.filter((line): line is TreeRow => isRecord(line)) }
}

/** What the page asks moai to find: the filter line as typed, the search, and whether the archive may answer. */
export interface Question {
  filter: string
  q: string
  archived: boolean
  /**
   * The worktree overlay is on (`SPC v w`): the rows are asked over it, as the list reads them (loadAllRows), so a row
   * only another worktree holds can match and an overlaid row is judged by the fields it is drawn with.
   */
  worktree?: boolean
}

/**
 * The reads a question takes: both lists with `fields=id` — except when the filter names a `type=`. `backlog show`
 * already picks the type and refuses another (`show_kind_typed`), while `show` answers a type for every kind, the
 * backlog's own included (`type=backlog`), so then `/api/rows` alone answers.
 */
export function matchEndpoints(question: Question): Endpoint[] {
  const query: Record<string, string> = { fields: 'id' }
  if (question.filter.trim() !== '') query.filter = question.filter
  if (question.q.trim() !== '') query.q = question.q.trim()
  if (question.archived) query.archived = '1'
  const typed = splitItems(question.filter).some((item) => item.split('=')[0]!.trim() === 'type')
  // The overlay is `/api/rows`' alone, as on the list's own read: the backlog has none.
  const rows: Endpoint = { path: ENDPOINT.rows, query: question.worktree ? { ...query, worktree: '1' } : query }
  return typed ? [rows] : [rows, { path: ENDPOINT.backlog, query }]
}

/**
 * The ids moai finds for a filter and a search ({@link matchEndpoints}). A refusal (a filter moai cannot read) is
 * the outcome, in moai's own words.
 */
export async function loadMatches(client: Client, question: Question, epoch?: number): Promise<Loaded<Set<string>>> {
  const outcomes = await Promise.all(matchEndpoints(question).map((endpoint) => client.request(endpoint, { priority: PRIORITY.list, epoch })))
  for (const outcome of outcomes) if (answerOf(outcome) === undefined) return { ok: false, outcome }
  const ids = new Set<string>()
  for (const outcome of outcomes) {
    const list = answerOf(outcome)
    if (!Array.isArray(list)) return { ok: false, outcome: { kind: 'unreadable' } }
    for (const row of items(list)) ids.add(row.id)
  }
  return { ok: true, value: ids }
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
 * Whether a board read that just ended should read the archive's rows. Only when that read was asked for the archive's
 * lines too (`askedWithArchive`, the view as the read began): `SPC v o` turned on while it ran joined a read asked
 * without them, and indexing the archive against a tree that lacks their lines puts those rows in the wrong place
 * until the follow-up read. That read (loadBoard sees the view moved) asks again with them and loads the archive then.
 */
export function archiveDue(askedWithArchive: boolean, shownNow: boolean, running: boolean, loaded: boolean): boolean {
  return askedWithArchive && shownNow && !running && !loaded
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

/** `moai wiki search <query>` as printed; the wiki view reads its order (moai-xp7l.nyi). */
export async function loadWikiSearch(client: Client, query: string): Promise<Loaded<unknown>> {
  const outcome = await client.request(ENDPOINT.wikiSearch(query), { priority: PRIORITY.wiki })
  const output = answerOf(outcome)
  return output === undefined ? { ok: false, outcome } : { ok: true, value: output }
}

/** `moai wiki show <slug>` as printed; the wiki view parses it. */
export async function loadWikiPage(client: Client, slug: string): Promise<Loaded<unknown>> {
  const outcome = await client.request(ENDPOINT.wikiPage(slug), { priority: PRIORITY.wiki })
  const output = answerOf(outcome)
  return output === undefined ? { ok: false, outcome } : { ok: true, value: output }
}

/**
 * One row with its body, children and journal (`moai show <id>`); `epoch` as client.ts RequestOptions says. A partial
 * answer is read like a whole one, as the lists are: one unreadable line anywhere in the tracker ends every command
 * non-zero with its whole answer still printed, and the detail of every row would otherwise fail beside a full list.
 */
export async function loadDetail(client: Client, id: string, options: Pick<RequestOptions, 'epoch'> & { worktree?: boolean } = {}): Promise<Loaded<Row>> {
  const outcome = await client.request(ENDPOINT.issue(id, options.worktree === true), { priority: PRIORITY.detail, epoch: options.epoch })
  const output = answerOf(outcome)
  if (output === undefined) return { ok: false, outcome }
  return isRecord(output) && typeof output.id === 'string' ? { ok: true, value: output as Row } : { ok: false, outcome: { kind: 'unreadable' } }
}

/**
 * The registered projects (`moai project ls --json`), read once after the boot. A failure is the outcome: the page then
 * draws no project column and binds no digit.
 */
export async function loadProjects(client: Client): Promise<Loaded<Project[]>> {
  const outcome = await client.request({ path: ENDPOINT.projects }, { priority: PRIORITY.background, cancellable: false })
  const output = answerOf(outcome)
  return output === undefined ? { ok: false, outcome } : { ok: true, value: projectsFrom(output) }
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
  /** The header's User line, already labelled with the project's naming as the TUI prints it; null when moai could
   *  not tell who is working, and when an older server does not send it. */
  user: string | null
  /** What moai last heard of the latest release (data/about.ts {@link Latest}); null when it holds none, and when an
   *  older server does not send it. */
  latest: Latest | null
  /** The TUI's zone (`[tui] timezone` as written), the page's first choice of zone (data/zone.ts); null when the TUI
   *  picked none, and when an older server does not send it. */
  timezone: string | null
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
  // `user` and `latest` came later: a server without them still boots, and the header then says `—`.
  const timezone = typeof value.timezone === 'string' && value.timezone !== '' ? value.timezone : null
  return { ...(value as unknown as Boot), user: userOf(value.user), latest: latestOf(value.latest), timezone }
}

/**
 * `/api/boot`, fetched outside the queue: nothing else can be read before it. An answer that does not come within
 * `timeoutMs` is the server out of reach, as a stamp's is (main.ts): nothing is drawn while the boot is waited on.
 */
export async function loadBoot(fetcher: Fetch, timeoutMs = 10_000): Promise<Loaded<Boot>> {
  let status: number
  let body: string
  const abort = new AbortController()
  const limit = setTimeout(() => abort.abort(), timeoutMs)
  try {
    const answer = await fetcher(ENDPOINT.boot, { signal: abort.signal, headers: { Accept: 'application/json' } })
    status = answer.status
    body = await answer.text()
  } catch {
    return { ok: false, outcome: { kind: 'not-started' } }
  } finally {
    clearTimeout(limit)
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
