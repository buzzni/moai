/**
 * Read marks: which rows are unread, and the writes that mark them read. No DOM here.
 *
 * Judgement (se-ehdi.bzp). The panel computes nothing. "Near me" (assigned to me, their `.children`, members
 * of my epics; milestones not counted) and `updated_at > seen` are decided by the CLI, which prints
 * `"unread": true` on `moai show --json` rows and on `moai show <id> --json` (upstream moai-cqwy, se-0g4a.l89;
 * released in moai 0.17.0). The key is there only when true and absent otherwise; with the person unknown it is
 * absent everywhere. So when no row carries it, NEW is simply off.
 *
 * Writes (se-ehdi.o5u). `moai read <id>...`, `--all`, `-e <group>` write the per-project file under
 * `~/.config/moai/read/`; nothing goes to the tracker. `--json` prints
 * `{"read":[ids written now],"missing":[ids not in the tracker],"held":[ids a hand-written value blocked],
 *   "problems":[prose],"at":"<when the command ran>"}`. Ids that were already read at their current
 * `updated_at` are not rewritten and so appear in none of `read`, `missing`, `held`. A missing id or group
 * (a row removed, or one only the archive holds) still writes the rest and prints that answer, but ends the run
 * non-zero; the answer is handed out all the same, with `missing` naming it.
 *
 * Stale draws. The design says the value written is the drawn row's `updated_at` (a row changed after it was
 * drawn comes back NEW). The CLI takes no value: it writes the row's *current* `updated_at`, and its result
 * carries no `updated_at` per id, so a stale draw CANNOT be detected from the write result; the changed row
 * is marked read for its newer version. What the panel can do is compare the drawn row with the row it
 * re-reads afterwards ({@link staleAgainst}); that shows the row changed, not whether before or after the
 * write. Callers re-read after every write.
 */
import { PRIORITY, answerOf, type Client, type Outcome } from './client.js'
import type { CompatProbe } from './compat.js'
import type { IssueIndex, Row } from './index.js'
import { PROFILE } from './source.js'

// ── judgement ────────────────────────────────────────────────────────

/**
 * The CLI's verdict: the key is `true` or absent. Archive-only rows (`archived: true`, printed by moai 0.17 on
 * the `show --archived` rows `show --all` hides, or tagged by source.ts withArchive as the index is built) never
 * count: `moai read` resolves active rows only, so their NEW could not be cleared (se-fvou).
 */
export const isUnread = (row: Row): boolean => row.unread === true && row.archived !== true

export const unreadIds = (rows: readonly Row[]): string[] => rows.filter(isUnread).map((row) => row.id)

export const unreadCount = (rows: readonly Row[]): number => rows.filter(isUnread).length

/** Did the source tell us anything about read state at all? When not, NEW (and any badge) stays off. */
export const knowsUnread = (rows: readonly Row[]): boolean => rows.some(isUnread)

/** Unread rows under a group (the group row included), as `SPC m g` would mark them. Zero for a non-group. */
export function unreadUnder(index: IssueIndex, groupId: string): number {
  return index.underGroup(groupId).filter((at) => isUnread(index.rows[at]!)).length
}

/** Unread rows below a place (a path of the index). */
export function unreadBelow(index: IssueIndex, path: readonly string[]): number {
  return index.descendants(path).filter((at) => isUnread(index.rows[at]!)).length
}

// ── optimistic update ────────────────────────────────────────────────

/**
 * Rows with `unread` cleared on `ids`, for drawing before the fresh re-read. Rows not named keep their identity;
 * the input is not changed.
 */
export function applyRead<R extends Row>(rows: readonly R[], ids: Iterable<string>): R[] {
  const named = new Set(ids)
  return rows.map((row) => {
    if (!named.has(row.id) || !('unread' in row)) return row
    const { unread: _unread, ...rest } = row
    return rest as R
  })
}

/**
 * Ids whose `updated_at` in `fresh` differs from the drawn row: they changed after (or while) they were drawn,
 * so what was marked is not what the person saw. A row missing from `fresh` is not reported.
 */
export function staleAgainst(drawn: readonly Row[], fresh: readonly Row[]): string[] {
  const now = new Map(fresh.map((row) => [row.id, row.updated_at]))
  return drawn.filter((row) => now.has(row.id) && now.get(row.id) !== row.updated_at).map((row) => row.id)
}

// ── writes ───────────────────────────────────────────────────────────

/** What `moai read --json` prints. */
export interface ReadResult {
  /** Ids written now. */
  read: string[]
  missing: string[]
  /** Ids blocked by a value a person wrote by hand; retry later. */
  held: string[]
  /** Prose reasons the CLI also printed on stderr. Empty is normal. */
  problems: string[]
  /** When the command ran (not a row's `updated_at`). */
  at: string
}

export type Marked = { ok: true; result: ReadResult } | { ok: false; outcome: Outcome }

const isRecord = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null
const strings = (value: unknown): string[] => (Array.isArray(value) ? value.filter((item): item is string => typeof item === 'string') : [])

/** Reads the `read --json` object; undefined when it is not one (`read` is the one required key). */
export function parseRead(output: unknown): ReadResult | undefined {
  if (!isRecord(output) || !Array.isArray(output.read)) return undefined
  return {
    read: strings(output.read),
    missing: strings(output.missing),
    held: strings(output.held),
    problems: strings(output.problems),
    at: typeof output.at === 'string' ? output.at : '',
  }
}

async function write(client: Client, profileId: string, value?: string, probe?: CompatProbe): Promise<Marked> {
  const outcome = await client.request(profileId, value, { priority: PRIORITY.detail, share: false })
  probe?.observe(profileId, outcome)
  // A missing id ends the run non-zero with the whole answer still printed (see the header).
  const result = parseRead(answerOf(outcome))
  if (result) return { ok: true, result }
  return { ok: false, outcome: outcome.kind === 'passed' ? { kind: 'unreadable' } : outcome }
}

/** `r`: the drawn row. Only its id travels; see the note on stale draws above. */
export const markRead = (client: Client, row: Pick<Row, 'id'>, probe?: CompatProbe): Promise<Marked> => write(client, PROFILE.read, row.id, probe)

/** `SPC m a`: everything unread that came to me. */
export const markAllRead = (client: Client, probe?: CompatProbe): Promise<Marked> => write(client, PROFILE.readAll, undefined, probe)

/** `SPC m g`: a group's members and what is drawn under it. */
export const markGroupRead = (client: Client, groupId: string, probe?: CompatProbe): Promise<Marked> => write(client, PROFILE.readGroup, groupId, probe)
