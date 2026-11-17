import type { Client } from './client.js'

export interface Entry { name: string; kind: 'dir' | 'file' | 'link' | 'other'; size: number; openable: boolean; dir?: boolean; inside?: boolean }
export interface FileText { path: string; text?: string; binary: boolean; truncated: boolean; spans?: Array<Array<[number, number, string]>> | null }
export interface SearchResult { matches: Array<{ path: string; start: number; end: number }>; total: number; truncated: boolean; walk_truncated: boolean; ignored_unavailable: boolean }
const record = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null
async function read(client: Client, path: string, query: Record<string, string>): Promise<unknown> {
  const result = await client.request({ path, query })
  if (result.kind !== 'passed') throw new Error(result.kind)
  return result.output
}
export function filesSource(client: Client) {
  return {
    async list(dir: string, limit?: number): Promise<{ entries: Entry[]; truncated: boolean }> {
      const v = await read(client, '/api/files', limit === undefined ? { dir } : { dir, limit: String(limit) })
      if (!record(v) || !Array.isArray(v.entries)) throw new Error('invalid directory')
      const entries = v.entries.filter((e): e is Entry => record(e) && typeof e.name === 'string' && e.name !== '.' && e.name !== '..' && !e.name.includes('/') && !e.name.includes('\\') && ['dir', 'file', 'link', 'other'].includes(String(e.kind)) && typeof e.openable === 'boolean' && typeof e.size === 'number')
      return { entries, truncated: v.truncated === true }
    },
    async file(path: string): Promise<FileText> {
      const v = await read(client, '/api/file', { path })
      if (!record(v) || typeof v.path !== 'string' || typeof v.binary !== 'boolean' || typeof v.truncated !== 'boolean' || (v.text !== undefined && typeof v.text !== 'string')) throw new Error('invalid file')
      return v as unknown as FileText
    },
    async search(q: string, dir: string): Promise<SearchResult> {
      const v = await read(client, '/api/find', { q, dir })
      if (!record(v) || !Array.isArray(v.matches) || !v.matches.every(m => record(m) && typeof m.path === 'string' && !m.path.startsWith('/') && !m.path.includes('\\') && !m.path.split('/').includes('..') && Number.isInteger(m.start) && Number.isInteger(m.end)) || typeof v.total !== 'number' || typeof v.truncated !== 'boolean' || typeof v.walk_truncated !== 'boolean' || typeof v.ignored_unavailable !== 'boolean') throw new Error('invalid search')
      return v as unknown as SearchResult
    },
    async ignored(): Promise<string[]> {
      const v = await read(client, '/api/ignored', {})
      return record(v) && Array.isArray(v.dirs) ? v.dirs.filter((d): d is string => typeof d === 'string') : []
    },
  }
}
