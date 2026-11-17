import type { Client } from './client.js'
import type { DiffAnswer } from '../view/diff.ts'
export interface Checkout { key: string; branch: string; main: boolean; milestone: string | null; issue: string | null; current: boolean }
/** The four letters the server folds a change into; anything else stands as '' (no letter, no colour). */
export type Mark = 'M' | 'A' | 'D' | 'R' | ''
export interface Change { path: string; old_path: string | null; status: string; mark: Mark; adds: number | null; dels: number | null; binary: boolean }
/** A checkout's change list as the page holds it: the files, each changed file's ancestor directories' letters, and how far the read got. */
export interface ChangeSet { files: Change[]; dirs: Array<[string, Mark]>; total: number; truncated: boolean; error?: string }
const LETTERS = new Set(['M', 'A', 'D', 'R'])
export const markOf = (v: unknown): Mark => typeof v === 'string' && LETTERS.has(v) ? v as Mark : ''
/** A path the server named, as the page lets it be compared with a row: relative, forward slashes, no `.`/`..`/empty part. */
export const safePath = (p: string): boolean => !p.startsWith('/') && !p.includes('\\') && !p.split('/').some(part => !part || part === '.' || part === '..')

export interface Entry { name: string; kind: 'dir' | 'file' | 'link' | 'other'; size: number; openable: boolean; dir?: boolean; inside?: boolean }
export interface FileText { stamp?: string | null; path: string; text?: string; binary: boolean; truncated: boolean; spans?: Array<Array<[number, number, string]>> | null }
export interface SearchResult { matches: Array<{ path: string; start: number; end: number }>; total: number; truncated: boolean; walk_truncated: boolean; ignored_unavailable: boolean }
const record = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null
export function changesOf(v: unknown): Change[] | null {
  if (!record(v) || !Array.isArray(v.files)) return null
  return v.files.filter((c): c is Change => record(c) && typeof c.path === 'string' && safePath(c.path) && typeof c.status === 'string' && (c.adds === null || typeof c.adds === 'number') && (c.dels === null || typeof c.dels === 'number'))
    .map(c => ({ ...c, mark: markOf((c as { mark?: unknown }).mark) }))
}
/** The answer of `/api/changes?root=`: files, the directories' map (keys sorted, bad keys and letters dropped), the read's reach. Null when it has no list. */
export function changeSetOf(v: unknown): ChangeSet | null {
  const files = changesOf(v); if (!files || !record(v)) return null
  const dirs: Array<[string, Mark]> = []
  if (record(v.dirs)) for (const key of Object.keys(v.dirs).sort()) { const mark = markOf(v.dirs[key]); if (mark && safePath(key)) dirs.push([key, mark]) }
  return { files, dirs, total: typeof v.total === 'number' ? v.total : files.length, truncated: v.truncated === true, ...(record(v.vcs_error) && typeof v.vcs_error.said === 'string' ? { error: v.vcs_error.said } : {}) }
}
/** One string for one answer: the watcher tells "same" from "changed" by it, so a directory's letter alone moves it. */
export const changeFingerprint = (set: ChangeSet): string => JSON.stringify(set)
export function validDiff(v: unknown): v is DiffAnswer {
  if (!record(v) || !Array.isArray(v.files) || typeof v.truncated !== 'boolean') return false
  return v.files.every(f => record(f) && typeof f.path === 'string' && typeof f.status === 'string' && typeof f.binary === 'boolean' &&
    (f.hunks === undefined || (Array.isArray(f.hunks) && f.hunks.every(h => record(h) &&
      typeof h.old_start === 'number' && typeof h.old_len === 'number' && typeof h.new_start === 'number' && typeof h.new_len === 'number' && typeof h.section === 'string' && Array.isArray(h.lines) &&
      h.lines.every(l => record(l) && ['context', 'add', 'delete', 'raw'].includes(String(l.kind)) && typeof l.text === 'string' && (l.spans == null || Array.isArray(l.spans)))))))
}
async function read(client: Client, path: string, query: Record<string, string>, background = false): Promise<unknown> {
  const result = await client.request({ path, query }, background ? { priority: 3, share: false, group: 'files-refresh' } : undefined)
  if (result.kind !== 'passed') throw new Error(result.kind)
  return result.output
}
export function filesSource(client: Client) {
  return {
    async list(dir: string, limit?: number, root?: string, background = false): Promise<{ entries: Entry[]; truncated: boolean; stamp: string | null }> {
      const query: Record<string, string> = { dir }; if (limit !== undefined) query.limit = String(limit); if (root !== undefined) query.root = root
      const v = await read(client, '/api/files', query, background)
      if (!record(v) || !Array.isArray(v.entries)) throw new Error('invalid directory')
      const entries = v.entries.filter((e): e is Entry => record(e) && typeof e.name === 'string' && e.name !== '.' && e.name !== '..' && !e.name.includes('/') && !e.name.includes('\\') && ['dir', 'file', 'link', 'other'].includes(String(e.kind)) && typeof e.openable === 'boolean' && typeof e.size === 'number')
      return { entries, truncated: v.truncated === true, stamp: typeof v.stamp === 'string' ? v.stamp : null }
    },
    async file(path: string, root?: string, background = false): Promise<FileText> {
      const v = await read(client, '/api/file', root === undefined ? { path } : { path, root }, background)
      if (!record(v) || typeof v.path !== 'string' || typeof v.binary !== 'boolean' || typeof v.truncated !== 'boolean' || (v.text !== undefined && typeof v.text !== 'string')) throw new Error('invalid file')
      return v as unknown as FileText
    },
    async search(q: string, dir: string, root?: string): Promise<SearchResult> {
      const v = await read(client, '/api/find', root === undefined ? { q, dir } : { q, dir, root })
      if (!record(v) || !Array.isArray(v.matches) || !v.matches.every(m => record(m) && typeof m.path === 'string' && !m.path.startsWith('/') && !m.path.includes('\\') && !m.path.split('/').includes('..') && Number.isInteger(m.start) && Number.isInteger(m.end)) || typeof v.total !== 'number' || typeof v.truncated !== 'boolean' || typeof v.walk_truncated !== 'boolean' || typeof v.ignored_unavailable !== 'boolean') throw new Error('invalid search')
      return v as unknown as SearchResult
    },
    async checkouts(): Promise<Checkout[]> {
      const v = await read(client, '/api/checkouts', {})
      if (!record(v) || !Array.isArray(v.checkouts)) throw new Error('invalid checkouts')
      return v.checkouts.filter((c): c is Checkout => record(c) && typeof c.key === 'string' && typeof c.branch === 'string' && typeof c.main === 'boolean' && typeof c.current === 'boolean' && (typeof c.issue === 'string' || c.issue === null))
    },
    async changes(root: string, background = false): Promise<ChangeSet> {
      const set = changeSetOf(await read(client, '/api/changes', { root }, background))
      if (!set) throw new Error('invalid changes')
      return set
    },
    async ignored(root?: string): Promise<string[]> {
      const v = await read(client, '/api/ignored', root === undefined ? {} : { root })
      return record(v) && Array.isArray(v.dirs) ? v.dirs.filter((d): d is string => typeof d === 'string') : []
    },
  }
}
