// A small `moai tui --json --deep` answer built from inline rows, for tests that only need rows placed under
// milestones and epics. It is a fixture builder, not moai's rule: the real answers are in fixtures/moai/tree*.json.
// A row stands under its id's parent row (`a.b` under `a`), else under its epic, else in the `none` bucket when the
// rows have milestones (at the root when they have none). An epic stands under its milestone.
export function treeOf(rows) {
  const byId = new Map(rows.map((row) => [row.id, row]))
  const hasMilestones = rows.some((row) => row.kind === 'milestone')
  const homeOf = (row) => {
    if (row.kind === 'milestone') return []
    const dot = row.id.lastIndexOf('.')
    const parent = dot > 0 ? byId.get(row.id.slice(0, dot)) : undefined
    if (parent) return [...homeOf(parent), parent.id]
    if (row.kind === 'epic' && row.milestone) return [row.milestone]
    const epic = row.derived_epic ?? row.epic
    if (row.kind !== 'epic' && epic && byId.has(epic)) return [...homeOf(byId.get(epic)), epic]
    if (row.milestone && byId.has(row.milestone)) return [row.milestone]
    return hasMilestones ? ['none'] : []
  }
  const homes = new Map(rows.map((row) => [row.id, homeOf(row)]))
  const parents = new Set([...homes.values()].flat())
  const lines = []
  const walk = (home) => {
    for (const row of rows) {
      const mine = homes.get(row.id)
      if (mine.join('/') !== home.join('/')) continue
      lines.push({
        id: row.id,
        title: row.title ?? row.id,
        kind: row.kind ?? 'issue',
        dir: parents.has(row.id),
        path: row.id,
        status: row.status ?? 'todo',
        priority: row.priority ?? 2,
        home: mine,
      })
      if (parents.has(row.id)) walk([...mine, row.id])
    }
    if (home.length === 0 && hasMilestones && rows.some((row) => homes.get(row.id)[0] === 'none')) {
      lines.push({ title: '(no milestone)', kind: 'bucket', dir: true, path: 'none', status: 'todo', priority: 2, home: [] })
      walk(['none'])
    }
  }
  walk([])
  return lines
}
