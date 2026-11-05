/** Present server groups, with an immediate local preview while a new tree is loading. */
import { isBacklog, isDone, isWork, type IssueIndex, type Keep, type Path, type Row, type TreeRow } from '../data/index.ts'
import { assigneeOf } from '../data/people.ts'
import type { SortPanelField } from './sort-panel.ts'

export interface Bucket { seg: string; label: string; members: number[] }
export function groupValue(group: string, row: Row, index: IssueIndex, at: number): string | null {
  switch (group) {
    case 'status': return row.status ?? null
    case 'assignee': return assigneeOf(row) || null
    case 'priority': return row.priority == null ? null : String(row.priority)
    case 'tag': return row.tags?.[0] ?? null
    case 'kind': return row.kind ?? null
    case 'milestone': return row.derived_milestone ?? row.milestone ?? index.homeOf(at).find((s) => s.startsWith('m:'))?.slice(2) ?? null
    case 'epic': return row.derived_epic ?? row.epic ?? index.epicOf(at) ?? null
    default: return null
  }
}
export function compareItems(index: IssueIndex, a: number, b: number, fields: readonly SortPanelField[], statuses: readonly string[] = []): number {
    const left = index.rows[a]!, right = index.rows[b]!
    for (const field of fields) {
      const value = (row: Row) => field.key === 'created' ? row.created_at : field.key === 'updated' ? row.updated_at : field.key === 'due' ? row.due_on : field.key === 'assignee' ? assigneeOf(row) || null : row[field.key]
      const x = value(left), y = value(right)
      if (x == null || y == null) { if (x !== y) return x == null ? 1 : -1; continue }
      const rank = (v: unknown) => { const at = statuses.indexOf(String(v)); return at < 0 ? statuses.length : at }
      const cmp = field.key === 'status' ? rank(x) - rank(y) : typeof x === 'number' && typeof y === 'number' ? x - y : String(x).localeCompare(String(y))
      if (cmp) return field.dir === 'desc' ? -cmp : cmp
    }
    return String(left.created_at ?? '').localeCompare(String(right.created_at ?? '')) || left.id.localeCompare(right.id)
  }
export function groupBuckets(index: IssueIndex, path: Path, keep: Keep, group: string, statuses: readonly string[] = [], includeClosed = false, order: readonly SortPanelField[] = [], levelOrder: readonly SortPanelField[] = [], server?: readonly TreeRow[]): Bucket[] {
  if (server) {
    const scope = new Set(index.descendants(path))
    const result: Bucket[] = []
    let current: Bucket | undefined
    for (const line of server) {
      if (line.kind === 'group' && line.path) {
        const payload = line.path.slice(line.path.indexOf(':', 6) + 1)
        let value: string | null
        try { value = JSON.parse(payload) as string | null } catch { continue }
        const ref = value === null || (group !== 'milestone' && group !== 'epic') ? undefined : index.find(value)
        const label = value === null ? '(none)' : group === 'priority' ? `p${value}` : group === 'tag' ? `#${value}` : ref === undefined ? value : index.rows[ref]!.title ?? value
        current = { seg: `group:${encodeURIComponent(group)}:${value === null ? 'n' : `v${encodeURIComponent(value)}`}`, label, members: [] }
        result.push(current)
      } else if (line.id) {
        const at = index.find(line.id)
        if (at === undefined || !scope.has(at) || !keep(at)) continue
        const row = index.rows[at]!
        if (!(isWork(row) || (isBacklog(row) && (includeClosed || !isDone(row))))) continue
        if (group === 'none' && !current) { current = { seg: 'group:none:n', label: '(none)', members: [] }; result.push(current) }
        current?.members.push(at)
      }
    }
    return result.filter((bucket) => bucket.members.length > 0)
  }
  const map = new Map<string | null, number[]>()
  for (const at of index.descendants(path)) {
    const row = index.rows[at]!
    if (index.find(row.id) !== at || !(isWork(row) || (isBacklog(row) && (includeClosed || !isDone(row)))) || !keep(at)) continue
    const value = groupValue(group, row, index, at)
    const members = map.get(value) ?? []
    members.push(at)
    map.set(value, members)
  }
  const values = [...map.keys()].sort((a, b) => {
    if (a === null || b === null) return a === b ? 0 : a === null ? 1 : -1
    if ((group === 'milestone' || group === 'epic') && levelOrder.length) {
      const ai = index.find(a), bi = index.find(b)
      if (ai !== undefined && bi !== undefined) {
        const ranked = compareItems(index, ai, bi, levelOrder, statuses)
        if (ranked) return ranked
      }
    }
    if (group === 'priority') return Number(a) - Number(b)
    if (group === 'status') {
      const rank = (v: string) => { const n = statuses.indexOf(v); return n < 0 ? statuses.length : n }
      return rank(a) - rank(b) || a.localeCompare(b)
    }
    return a.localeCompare(b)
  })

  return values.map((value) => {
    const ref = value === null || (group !== 'milestone' && group !== 'epic') ? undefined : index.find(value)
    const label = value === null ? '(none)' : group === 'priority' ? `p${value}` : group === 'tag' ? `#${value}` : ref === undefined ? value : index.rows[ref]!.title ?? value
    return { seg: `group:${encodeURIComponent(group)}:${value === null ? 'n' : `v${encodeURIComponent(value)}`}`, label, members: map.get(value)!.sort((a, b) => compareItems(index, a, b, order, statuses)) }
  })
}
export function groupLabel(seg: string): string | null {
  if (!seg.startsWith('group:')) return null
  const value = seg.split(':')[2] ?? 'n'
  return value === 'n' ? '(none)' : decodeURIComponent(value.slice(1))
}
