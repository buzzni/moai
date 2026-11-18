import type { Client, Outcome } from './client.ts'
import { changesOf, validDiff, type Change } from './files.ts'
import type { DiffAnswer } from '../view/diff.ts'
export type Scope = 'commits' | 'branch' | 'working'
export interface Offer { scope: Scope; checkout: string | null; branch: string | null; owner: string | null }
/** An issue's change list: the scopes it offers, the one chosen, and that scope's files with their `+n −n` (no hunks). */
export interface IssueChanges { id: string; scope: Scope | null; offers: Offer[]; files: Change[]; total: number; truncated: boolean; vcs_error: { said: string } | null }
/**
 * A read the pane tells: its value, or why not. `timeout` is the server's 504 (it gave up), `cancelled` a read the
 * person cancelled or the page dropped, `failed` any other — `said` the server's own words when it gave some.
 */
export type Read<T> = { ok: true; value: T } | { ok: false; why: 'timeout' | 'cancelled' | 'failed'; said?: string }
const record = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null
const SCOPES = ['commits', 'branch', 'working']
export function whyNot(outcome: Outcome): Read<never> {
  if (outcome.kind === 'cancelled') return { ok: false, why: 'cancelled' }
  if (outcome.kind === 'moai-error' && outcome.code === 'timeout') return { ok: false, why: 'timeout' }
  return outcome.kind === 'moai-error' ? { ok: false, why: 'failed', said: outcome.message } : { ok: false, why: 'failed' }
}
/** The patch reads of one pane go in this group: a pane that moves on cancels them all at once (client.cancelGroup). */
const options = (group: string, background: boolean) => ({ priority: background ? 3 : 0.5, group, ...(background ? { share: false } : {}) })

/**
 * The first step of an issue's diff: `/api/changes?id=…[&scope=…]`. Without a scope the server picks the one
 * `/api/diff` would show and lists it, so every file's head stands from this one read before any patch is asked.
 * The paths pass `changesOf` (no absolute path, `..` or `\`): only these are ever sent back as `path`.
 */
export async function issueChanges(client: Client, id: string, scope?: Scope, worktree = false, background = false): Promise<Read<IssueChanges>> {
  const query: Record<string, string> = { id }
  if (scope) query.scope = scope
  if (worktree) query.worktree = '1'
  const result = await client.request({ path: '/api/changes', query }, { priority: background ? 3 : 0.5, ...(background ? { share: false, group: 'files-refresh' } : {}) })
  if (result.kind !== 'passed') return whyNot(result)
  const v = result.output
  if (!record(v) || v.id !== id || !Array.isArray(v.files) || !Array.isArray(v.offers) || !v.offers.every(o => record(o) && SCOPES.includes(String(o.scope))) || !(v.scope === null || v.scope === undefined || SCOPES.includes(String(v.scope)))) return { ok: false, why: 'failed' }
  const error = record(v.vcs_error) && typeof v.vcs_error.said === 'string' ? { said: v.vcs_error.said } : null
  const files = changesOf(v) ?? []
  return { ok: true, value: { id, scope: (v.scope ?? null) as Scope | null, offers: v.offers as unknown as Offer[], files, total: typeof v.total === 'number' ? v.total : files.length, truncated: v.truncated === true, vcs_error: error } }
}

/**
 * The second step: one file's patch, `/api/diff?id=…&scope=…&path=…`. `path` is a row of the list the server gave
 * for this issue and scope — never a name the page made or a person typed.
 */
export async function issuePatch(client: Client, id: string, scope: Scope, path: string, worktree = false, background = false): Promise<Read<DiffAnswer>> {
  const query: Record<string, string> = { id, scope, path }
  if (worktree) query.worktree = '1'
  const result = await client.request({ path: '/api/diff', query }, options(PATCH_GROUP.issue, background))
  if (result.kind !== 'passed') return whyNot(result)
  return validDiff(result.output) ? { ok: true, value: result.output } : { ok: false, why: 'failed' }
}
/** One checkout's file patch, `/api/diff?root=…&path=…` (the files tab); `path` comes from that checkout's change list. */
export async function checkoutPatch(client: Client, root: string, path: string, background = false): Promise<Read<DiffAnswer>> {
  const result = await client.request({ path: '/api/diff', query: { root, path } }, options(PATCH_GROUP.files, background))
  if (result.kind !== 'passed') return whyNot(result)
  return validDiff(result.output) ? { ok: true, value: result.output } : { ok: false, why: 'failed' }
}
export const PATCH_GROUP = { issue: 'diff-patch', files: 'files-patch' } as const

/** A queued patch read: `live` is asked again when its turn comes, and a read no longer wanted is skipped unsent. */
export interface PatchJob { live(): boolean; run(): Promise<void> }
/**
 * The pane's own line of patch reads: at most `limit` at once (two — of the client's three, one stays for the list
 * and the detail), the rest in the order they were asked. `clear` drops what waits; what runs is the client's to
 * cancel (`cancelGroup`), and its answer meets a dead ticket.
 */
export function patchQueue(limit = 2) {
  const waiting: PatchJob[] = []
  let running = 0
  const pump = () => {
    while (running < limit) {
      const job = waiting.shift()
      if (!job) return
      if (!job.live()) continue
      running++
      void job.run().catch(() => {}).finally(() => { running--; pump() })
    }
  }
  return {
    add(job: PatchJob) { waiting.push(job); pump() },
    clear() { waiting.length = 0 },
    running: () => running,
    waiting: () => waiting.length,
  }
}
