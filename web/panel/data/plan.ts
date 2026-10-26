/**
 * The gantt's one write to the tracker: a row's planned days (moai-d86e.3ud). No DOM here.
 *
 * The write carries the move, not the days (moai-vj8w.02c): `POST /api/plan/<id>?edge=<start|body|end>&days=<N>` runs
 * `moai edit --shift-start=N | --shift=N | --shift-due=N -- <id>`, and `?day=<day>` runs
 * `moai edit --start=<day> --due=<day> --if-unplanned -- <id>` (src/web/api.rs `write_argv_of`). moai measures the days
 * inside its write lock from the row as it stands then (`Issue::shifted`), the way the TUI's drag does — so a row
 * another session moved after the page read it moves on from there instead of being given the days drawn here, and a
 * day planned on a row that gained days meanwhile is refused rather than written over them. The write goes the CLI's
 * way — `store::with_write`, the same normalising and the same refusals (a start after the deadline, a day past the
 * deadline above, no days to move).
 *
 * view/gantt-model.ts `shiftPlan` still draws the bar while it is dragged and says why a press moves nothing; it never
 * decides what is written.
 */
import { PRIORITY, type Client, type Endpoint, type Outcome } from './client.js'

/** What to write: a move of the plan by whole days, or one day planned on a row with none. */
export type PlanChange = { edge: 'start' | 'body' | 'end'; days: number } | { day: string }

export const planEndpoint = (id: string, change: PlanChange): Endpoint => ({
  path: `/api/plan/${encodeURIComponent(id)}`,
  method: 'POST',
  query: 'day' in change ? { day: change.day } : { edge: change.edge, days: String(change.days) },
})

/** `said`: what moai wrote on stderr beside the answer — the rows a pulled-in deadline now overtakes. */
export type Planned = { ok: true; said?: string[] } | { ok: false; outcome: Outcome }

export async function writePlan(client: Client, id: string, change: PlanChange): Promise<Planned> {
  // Never shared, and Cancel does not stop it: a write of its own, as a read mark is (data/read.ts).
  const outcome = await client.request(planEndpoint(id, change), { priority: PRIORITY.detail, share: false })
  if (outcome.kind === 'passed') return { ok: true, ...(outcome.notes ? { said: outcome.notes } : {}) }
  return { ok: false, outcome }
}

/**
 * The `moai edit` line that puts the days back, as the TUI's notice names it. A day never written is `none`. Built from
 * the days the page read: moai's answer carries the row after the write, not before it.
 */
export const undoLine = (id: string, start: string | null | undefined, due: string | null | undefined): string =>
  `moai edit ${id} --start ${start || 'none'} --due ${due || 'none'}`
