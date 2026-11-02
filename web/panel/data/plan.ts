/**
 * The gantt's one write to the tracker: a row's planned days (moai-d86e.3ud). No DOM here.
 *
 * Ordinary writes carry the move, not the days (moai-vj8w.02c): `POST /api/plan/<id>?edge=<start|body|end>&days=<N>` runs
 * `moai edit --shift-start=N | --shift=N | --shift-due=N -- <id>`, and `?day=<day>` runs
 * `moai edit --start=<day> --due=<day> --if-unplanned -- <id>` (src/web/api.rs `write_argv_of`). moai measures the days
 * inside its write lock from the row as it stands then (`Issue::shifted`), the way the TUI's drag does — so a row
 * another session moved after the page read it moves on from there instead of being given the days drawn here, and a
 * day planned on a row that gained days meanwhile is refused rather than written over them. The write goes the CLI's
 * way — `store::with_write`, the same normalising and the same refusals (a start after the deadline, a day past the
 * deadline above, no days to move).
 *
 * A filled start dragged to a date sends `?start=<day>`, translated to `moai edit --start=<day>`. This explicit choice
 * overwrites any start written meanwhile (option D); the existing CLI write still validates and locks it.
 *
 * view/gantt-model.ts `shiftPlan` still draws the bar while it is dragged and says why a press moves nothing; a filled-start resize writes the chosen absolute start.
 */
import { PRIORITY, type Client, type Endpoint, type Outcome } from './client.js'

/** A relative move, a new one-day plan, or an absolute start chosen from a filled edge. */
export type PlanChange = { edge: 'start' | 'body' | 'end'; days: number } | { day: string } | { start: string }

export const planEndpoint = (id: string, change: PlanChange): Endpoint => ({
  path: `/api/plan/${encodeURIComponent(id)}`,
  method: 'POST',
  query: 'day' in change ? { day: change.day } : 'start' in change ? { start: change.start } : { edge: change.edge, days: String(change.days) },
})

/** `said`: what moai wrote on stderr beside the answer — the rows a pulled-in deadline now overtakes. */
/** `undo` is computed from the locked before/after pair and names the tracker with -C. */
export type Planned = { ok: true; undo?: string; said?: string[] } | { ok: false; outcome: Outcome }

export async function writePlan(client: Client, id: string, change: PlanChange): Promise<Planned> {
  // Never shared, and Cancel does not stop it: a write of its own, as a read mark is (data/read.ts).
  const outcome = await client.request(planEndpoint(id, change), { priority: PRIORITY.detail, share: false })
  if (outcome.kind === 'passed') {
    const out = outcome.output
    const undo = typeof out === 'object' && out !== null && 'undo' in out && typeof out.undo === 'string' ? out.undo : undefined
    return { ok: true, ...(undo ? { undo } : {}), ...(outcome.notes ? { said: outcome.notes } : {}) }
  }
  return { ok: false, outcome }
}
