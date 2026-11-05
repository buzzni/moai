/** Editor state and labels for the shared engine vocabulary. */
import { SORT_KEYS, defaultDir, type SortField, type SortKey } from '../data/query.ts'
import type { SortPanelState, SortPanelVocabulary } from './sort-panel.ts'

type EngineWords = { sort_layers?: string[]; sort_groups?: string[]; sort_fields: { name: string; dir: 'asc' | 'desc' }[]; sort_presets?: { name: string; order: string; group: string }[] }
let engine: EngineWords | null = null
export function useSortVocabulary(words: EngineWords): void { engine = words }
const f = (key: SortKey, dir = defaultDir(key)): SortField => ({ key, dir })
export function preset(id: string): SortPanelState {
  const supplied = engine?.sort_presets?.find((p) => p.name === id)
  if (supplied) {
    const order: Record<string, SortField[]> = {}
    for (const clause of supplied.order.split(';')) {
      const [level, raw] = clause.split('=')
      if (!level || !raw) continue
      order[level] = raw.split(',').map((word) => {
        const [key, ...modifiers] = word.split(':')
        const base = engine?.sort_fields.find((field) => field.name === key)?.dir ?? 'asc'
        const dir = modifiers.includes('desc') ? 'desc' : modifiers.includes('asc') ? 'asc' : base
        return { key: key as SortKey, dir, nullsFirst: modifiers.includes('nulls-first') }
      })
    }
    return { group: supplied.group, order, level: supplied.group === 'structure' ? 'milestone' : 'item', preset: id, edited: true }
  }
  const order = { milestone: [f('due'), f('created')], epic: [f('priority'), f('created', 'asc')], item: [f('priority'), f('due')] }
  let group = 'structure'
  if (id === 'urgent') { group = 'none'; order.epic = [f('priority')] }
  if (id === 'recent') { group = 'none'; order.milestone = [f('updated')]; order.epic = [f('updated')]; order.item = [f('updated')] }
  if (id === 'due') { group = 'none'; order.milestone = [f('due')]; order.epic = [f('due')]; order.item = [f('due'), f('priority')] }
  if (id === 'people') { group = 'assignee'; order.epic = [f('priority')] }
  return { group, order, level: group === 'structure' ? 'milestone' : 'item', preset: id, edited: true }
}
/** Every field the sort window lists, in the vocabulary's order (`/api/boot`, else the page's own keys). */
export function sortFieldNames(): string[] {
  return engine ? engine.sort_fields.map((field) => field.name) : [...SORT_KEYS]
}
/** A field's own direction, as it is put in. */
export function fieldDir(key: string): 'asc' | 'desc' {
  return engine?.sort_fields.find((field) => field.name === key)?.dir ?? defaultDir(key as SortKey)
}
/** The sort window's rows for one level, top to bottom: the level's fields in their order, then the rest. */
export function sorterRows(panel: SortPanelState): string[] {
  const used = (panel.order[panel.level] ?? []).map((field) => field.key)
  return [...used, ...sortFieldNames().filter((key) => !used.includes(key) && (panel.level !== 'item' || key !== 'progress'))]
}
/** The groups `g` cycles through, as the panel lists them. */
export function groupNames(): string[] {
  const all = ['structure', 'none', 'status', 'assignee', 'priority', 'tag', 'kind', 'milestone', 'epic']
  return engine?.sort_groups ?? all
}
export function activeLevels(group: string): string[] {
  return group === 'structure' ? ['milestone', 'epic', 'item'] : group === 'milestone' ? ['milestone', 'item'] : group === 'epic' ? ['epic', 'item'] : ['item']
}
export function vocabulary(lang: string, names: Record<SortKey, string>): SortPanelVocabulary {
  const ko = lang === 'ko'
  const label = (en: string, korean: string) => ko ? korean : en
  const pairs = (items: readonly (readonly [string, string, string])[]) => items.map(([id, en, korean]) => ({ id, label: label(en, korean) }))
  return {
    presets: pairs([['release', 'Release', '판 차례'], ['urgent', 'Urgent', '급한 것 먼저'], ['recent', 'Recent', '최근 바뀐 것'], ['due', 'Due', '기한 임박'], ['people', 'People', '담당별']]),
    groups: pairs([['structure', 'Structure', '구조'], ['none', 'None', '없음'], ['status', 'Status', '칸'], ['assignee', 'Assignee', '담당'], ['priority', 'Priority', '우선순위'], ['tag', 'Tag', '태그'], ['kind', 'Kind', '종류'], ['milestone', 'Milestone', '마일스톤'], ['epic', 'Epic', '에픽']]).filter((item) => !engine?.sort_groups || engine.sort_groups.includes(item.id)),
    levels: pairs([['milestone', 'Milestone', '마일스톤'], ['epic', 'Epic', '에픽'], ['item', 'Item', '일']]).filter((item) => !engine?.sort_layers || engine.sort_layers.includes(item.id)),
    fields: engine ? engine.sort_fields.map(({ name, dir }) => ({ id: name, label: names[name as SortKey] ?? name, dir, ...(name === 'progress' ? { levels: ['milestone', 'epic'] } : {}) })) : SORT_KEYS.map((id) => ({ id, label: names[id] ?? id, dir: defaultDir(id) })), activeLevels,
    labels: { presets: label('Preset', '프리셋'), groups: label('Group', '묶기'), order: label('Order', '차례'), add: label('Add field', '필드 더하기'), direction: label('Reverse', '방향 뒤집기'), up: label('Move up', '위로'), down: label('Move down', '아래로'), remove: label('Remove', '빼기'), command: label('Command', '명령'), speech: label('Say', '말'), hint: label('Empty values last; ties: created, then id.', '빈 값은 끝, 같으면 만든 때와 ID로 가른다.'), saveDefault: label('Set as default', '기본으로') },
  }
}

/** Start from the order the existing engine actually returned. */
export function initialPanel(): SortPanelState {
  const panel = preset('release')
  panel.order.item = [f('priority')]
  panel.level = 'item'
  panel.edited = false
  panel.preset = undefined
  return panel
}

export function describe(panel: SortPanelState, words: SortPanelVocabulary): { summary: string; command: string; speech: string } {
  const levels = activeLevels(panel.group)
  const group = words.groups.find((x) => x.id === panel.group)?.label ?? panel.group
  const pieces = levels.map((level) => {
    const name = words.levels.find((x) => x.id === level)?.label ?? level
    const fields = (panel.order[level] ?? []).map((field) => `${words.fields.find((x) => x.id === field.key)?.label ?? field.key}${field.dir === 'asc' ? '↑' : '↓'}`).join(', ')
    return `${name} ${fields}`
  })
  const expression = orderExpression(panel)
  const presetName = words.presets.find((x) => x.id === panel.preset)?.label
  const groupCommand = !panel.preset || panel.group !== 'structure' ? `\nmoai ctl group ${panel.group}` : ''
  const ko = words.labels.groups === '묶기'
  return { summary: presetName ?? `${words.labels.groups}: ${group} · ${pieces.join(' · ')}`,
    command: `moai ctl sort '${panel.preset ?? expression}'${groupCommand}\nmoai show --group ${panel.group} --sort '${panel.preset ?? expression}'`,
    speech: ko ? `${group}별로 묶고, ${pieces.join(', ')} 순으로 정렬해줘` : `Group by ${group}, then sort ${pieces.join(', ')}.` }

}

/** Parse the engine's layered wire expression. The engine still validates the resulting query. */
export function parsePanelOrder(raw: string): SortPanelState | null {
  const text = raw.trim()
  if (!text) return null
  if ((engine?.sort_presets ?? []).some((item) => item.name === text) || ['release', 'urgent', 'recent', 'due', 'people'].includes(text)) return preset(text)
  const allowed = new Set(engine?.sort_fields.map((field) => field.name) ?? ['priority', 'created', 'updated', 'status', 'assignee', 'title', 'due', 'id', 'kind', 'milestone', 'epic', 'tag', 'started', 'done', 'starts', 'since', 'progress'])
  const readFields = (input: string): import('./sort-panel.ts').SortPanelField[] | null => {
    const result: import('./sort-panel.ts').SortPanelField[] = []
    for (const word of input.split(',')) {
      const [key, ...modifiers] = word.trim().split(':')
      if (!key || !allowed.has(key) || result.some((field) => field.key === key)) return null
      if (modifiers.some((m) => !['asc', 'desc', 'nulls-first'].includes(m)) || modifiers.filter((m) => m === 'asc' || m === 'desc').length > 1 || modifiers.filter((m) => m === 'nulls-first').length > 1) return null
      const own = engine?.sort_fields.find((field) => field.name === key)?.dir ?? (key === 'created' || key === 'updated' ? 'desc' : 'asc')
      result.push({ key, dir: modifiers.includes('desc') ? 'desc' : modifiers.includes('asc') ? 'asc' : own, ...(modifiers.includes('nulls-first') ? { nullsFirst: true } : {}) })
    }
    return result.length ? result : null
  }
  const panel = preset('release')
  panel.preset = undefined
  if (!text.includes('=')) {
    const fields = readFields(text)
    if (!fields) return null
    panel.order = { milestone: fields.map((f) => ({ ...f })), epic: fields.map((f) => ({ ...f })), item: fields.map((f) => ({ ...f })) }
  } else {
    const seen = new Set<string>()
    for (const clause of text.split(';')) {
      const [level, input, extra] = clause.split('=')
      if (!level || !input || extra !== undefined || !['milestone', 'epic', 'item'].includes(level) || seen.has(level)) return null
      const fields = readFields(input)
      if (!fields) return null
      seen.add(level); panel.order[level] = fields
    }
  }
  return panel
}
export function groupKnown(group: string): boolean {
  return (engine?.sort_groups ?? ['structure', 'none', 'status', 'assignee', 'priority', 'tag', 'kind', 'milestone', 'epic']).includes(group)
}
export function orderExpression(panel: SortPanelState): string {
  return ['milestone', 'epic', 'item'].map((level) => `${level}=${(panel.order[level] ?? []).map((field) => `${field.key}${field.dir === fieldDir(field.key) ? '' : `:${field.dir}`}${field.nullsFirst ? ':nulls-first' : ''}`).join(',')}`).join(';')
}
