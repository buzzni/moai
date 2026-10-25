/**
 * The gantt's one write to the tracker: a row's planned days (moai-d86e.3ud). No DOM here.
 *
 * `POST /api/plan/<id>?start=<day>&due=<day>` runs `moai edit --start=<day> --due=<day> -- <id>` (src/web/api.rs
 * `write_argv_of`), so the write goes the CLI's way — `store::with_write`, the same normalising and the same refusals
 * (a start after the deadline, a day past the deadline above). Only the side that changes travels: a side left out is
 * left as it is, and rewriting a side the page could not read would wipe what is written there. Which side changes is
 * view/gantt-model.ts `shiftPlan`, the page's copy of `Issue::shifted`.
 *
 * The write carries days, not a shift: a row another session moved after it was drawn is given the days drawn here.
 * The rows are read again every couple of seconds (data/refresh.ts) and after every write, so the window is that short.
 */
import { PRIORITY, type Client, type Endpoint, type Outcome } from './client.js'

/** The days to write: `YYYY-MM-DD`, the changing side only. */
export interface PlanChange {
  start?: string
  due?: string
}

export const planEndpoint = (id: string, change: PlanChange): Endpoint => ({
  path: `/api/plan/${encodeURIComponent(id)}`,
  method: 'POST',
  query: { ...(change.start ? { start: change.start } : {}), ...(change.due ? { due: change.due } : {}) },
})

/** `said`: what moai wrote on stderr beside the answer — the rows a pulled-in deadline now overtakes. */
export type Planned = { ok: true; said?: string[] } | { ok: false; outcome: Outcome }

export async function writePlan(client: Client, id: string, change: PlanChange): Promise<Planned> {
  // Never shared, and Cancel does not stop it: a write of its own, as a read mark is (data/read.ts).
  const outcome = await client.request(planEndpoint(id, change), { priority: PRIORITY.detail, share: false })
  if (outcome.kind === 'passed') return { ok: true, ...(outcome.notes ? { said: outcome.notes } : {}) }
  return { ok: false, outcome }
}

/** The `moai edit` line that puts the days back, as the TUI's notice names it. A day never written is `none`. */
export const undoLine = (id: string, start: string | null | undefined, due: string | null | undefined): string =>
  `moai edit ${id} --start ${start || 'none'} --due ${due || 'none'}`
