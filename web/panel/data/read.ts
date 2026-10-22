/**
 * Read marks: which rows are unread, and the writes that mark them read. No DOM here.
 *
 * Judgement (se-ehdi.bzp). The page computes nothing. "Near me" (assigned to me, their `.children`, members
 * of my epics; milestones not counted) and `updated_at > seen` are decided by the CLI, which prints
 * `"unread": true` on `moai show --json` rows and on `moai show <id> --json`. The key is there only when true and
 * absent otherwise; with the person unknown it is absent everywhere. So when no row carries it, NEW is simply off.
 *
 * Writes (se-ehdi.o5u, moai-ly88). `POST /api/read/<id>`, `/api/read-all`, `/api/read-group/<id>` run `moai read -- <id>`,
 * `read --all`, `read --epic=<id>`, which write the per-person file beside the user config; nothing goes to the
 * tracker. The envelope's `out` is `read --json`:
 * `{"read":[ids written now],"missing":[ids not in the tracker],"held":[ids a hand-written value blocked],
 *   "problems":[prose],"at":"<when the command ran>"}`. Ids that were already read at their current
 * `updated_at` are not rewritten and so appear in none of `read`, `missing`, `held`. A missing id or group
 * (a row removed, or one only the archive holds) still writes the rest and prints that answer, but ends the run
 * non-zero; the answer is handed out all the same, with `missing` naming it.
 *
 * Stale draws. The CLI takes no value: it writes the row's *current* `updated_at`, so a row changed after it was drawn
 * is marked read for its newer version. Callers read the rows again after every write (main.ts marked()).
 */
import { PRIORITY, answerOf, type Client, type Endpoint, type Outcome } from './client.js'
import type { Row } from './index.js'

export const isUnread = (row: Row): boolean => row.unread === true && row.archived !== true

export const unreadIds = (rows: readonly Row[]): string[] => rows.filter(isUnread).map((row) => row.id)

// ── writes ───────────────────────────────────────────────────────────

/** The three writes `moai web` takes (src/web/api.rs write_argv_of); no body, no query of their own. */
export const READ_ENDPOINT = {
  one: (id: string): Endpoint => ({ path: `/api/read/${encodeURIComponent(id)}`, method: 'POST' }),
  all: { path: '/api/read-all', method: 'POST' } as Endpoint,
  group: (id: string): Endpoint => ({ path: `/api/read-group/${encodeURIComponent(id)}`, method: 'POST' }),
} as const

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

async function write(client: Client, endpoint: Endpoint): Promise<Marked> {
  // Never shared: a mark is a write of its own (client.ts shares no POST either).
  const outcome = await client.request(endpoint, { priority: PRIORITY.detail, share: false })
  // A missing id ends the run non-zero with the whole answer still printed (see the header).
  const result = parseRead(answerOf(outcome))
  if (result) return { ok: true, result }
  return { ok: false, outcome: outcome.kind === 'passed' ? { kind: 'unreadable' } : outcome }
}

/** `r`: the drawn row. Only its id travels; see the note on stale draws above. */
export const markRead = (client: Client, row: Pick<Row, 'id'>): Promise<Marked> => write(client, READ_ENDPOINT.one(row.id))

/** `SPC m a`: everything unread that came to me. */
export const markAllRead = (client: Client): Promise<Marked> => write(client, READ_ENDPOINT.all)

/** `SPC m g`: a group's members and what is drawn under it. */
export const markGroupRead = (client: Client, groupId: string): Promise<Marked> => write(client, READ_ENDPOINT.group(groupId))

/**
 * The three writes as the keys call them (keys/wiring.ts `r`, `SPC m a`, `SPC m g`), each answering null when it went
 * through or the {@link Outcome} that stopped it. `after` runs once a write went through, with the ids it wrote, and
 * is waited for: the page reads the rows again there (main.ts), so the marks come back with them.
 */
export interface ReadMarks {
  /** `r`: the row under the cursor. */
  one(id: string): Promise<Outcome | null>
  /** `SPC m a`: everything unread. */
  all(): Promise<Outcome | null>
  /** `SPC m g`: a milestone's or epic's members. */
  group(id: string): Promise<Outcome | null>
}

export function readMarks(client: Client, after: (written: ReadonlySet<string>) => Promise<unknown>): ReadMarks {
  const marked = async (writing: Promise<Marked>): Promise<Outcome | null> => {
    const done = await writing
    if (!done.ok) return done.outcome
    await after(new Set(done.result.read))
    return null
  }
  return {
    one: (id) => marked(markRead(client, { id })),
    all: () => marked(markAllRead(client)),
    group: (id) => marked(markGroupRead(client, id)),
  }
}
