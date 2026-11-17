import type { Client } from './client.ts'
import { validDiff } from './files.ts'
import type { DiffAnswer } from '../view/diff.ts'
export type Scope = 'commits' | 'branch' | 'working'
export interface Offer { scope: Scope; checkout: string | null; branch: string | null; owner: string | null }
export interface IssueDiff extends DiffAnswer { scope: Scope | null; offers: Offer[] }
const record = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null
export async function issueDiff(client: Client, id: string, scope?: Scope, worktree = false, background = false): Promise<IssueDiff> {
  const query: Record<string, string> = { id }
  if (scope) query.scope = scope
  if (worktree) query.worktree = '1'
  const result = await client.request({ path: '/api/diff', query }, { priority: background ? 3 : 0.5, ...(background ? { share: false, group: 'files-refresh' } : {}) })
  if (result.kind !== 'passed') throw new Error(result.kind)
  const v = result.output
  if (!validDiff(v) || !record(v) || v.id !== id || !Array.isArray(v.offers) || !v.offers.every(o => record(o) && ['commits', 'branch', 'working'].includes(String(o.scope))) || !(v.scope === null || ['commits', 'branch', 'working'].includes(String(v.scope)))) throw new Error('invalid diff')
  return v as unknown as IssueDiff
}
