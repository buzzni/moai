/** Present the grouped tree in the order supplied by the server. */
import { isBacklog, isDone, isWork, type IssueIndex, type Keep, type Path, type TreeRow } from '../data/index.ts'
import type { SortPanelField } from './sort-panel.ts'

export interface Bucket { seg: string; label: string; members: number[] }
export function groupBuckets(index: IssueIndex, path: Path, keep: Keep, group: string, _statuses: readonly string[] = [], includeClosed = false, _order: readonly SortPanelField[] = [], _levelOrder: readonly SortPanelField[] = [], server?: readonly TreeRow[]): Bucket[] {
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
        const label = value === null ? '(none)' : group === 'tag' ? `#${value}` : ref === undefined ? value : index.rows[ref]!.title ?? value
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
  return []
}
export function groupLabel(seg: string): string | null {
  if (!seg.startsWith('group:')) return null
  const value = seg.split(':')[2] ?? 'n'
  return value === 'n' ? '(none)' : decodeURIComponent(value.slice(1))
}
