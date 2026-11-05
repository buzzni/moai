/** Temporary UI adapter. The engine's boot vocabulary and query replace this at integration. */
import { SORT_KEYS, defaultDir, type SortField, type SortKey } from '../data/query.ts'
import type { SortPanelState, SortPanelVocabulary } from './sort-panel.ts'

const f = (key: SortKey, dir = defaultDir(key)): SortField => ({ key, dir })
export function preset(id: string): SortPanelState {
  const order = { milestone: [f('due'), f('created')], epic: [f('priority'), f('created', 'asc')], item: [f('priority'), f('due')] }
  let group = 'structure'
  if (id === 'urgent') { group = 'none'; order.epic = [f('priority')] }
  if (id === 'recent') { group = 'none'; order.milestone = [f('updated')]; order.epic = [f('updated')]; order.item = [f('updated')] }
  if (id === 'due') { group = 'none'; order.milestone = [f('due')]; order.epic = [f('due')]; order.item = [f('due'), f('priority')] }
  if (id === 'people') { group = 'assignee'; order.epic = [f('priority')] }
  return { group, order, level: group === 'structure' ? 'milestone' : 'item', preset: id, edited: true }
}
export function activeLevels(group: string): string[] {
  return group === 'structure' ? ['milestone', 'epic', 'item'] : group === 'milestone' ? ['milestone', 'item'] : group === 'epic' ? ['epic', 'item'] : ['item']
}
export function vocabulary(lang: string, names: Record<SortKey, string>): SortPanelVocabulary {
  const ko = lang === 'ko'
  const label = (en: string, korean: string) => ko ? korean : en
  const pairs = (items: readonly (readonly [string, string, string])[]) => items.map(([id, en, korean]) => ({ id, label: label(en, korean) }))
  return {
    presets: pairs([['release', 'Release', '판 차례'], ['urgent', 'Urgent', '급한 것 먼저'], ['recent', 'Recent', '최근 변경'], ['due', 'Due', '기한 임박'], ['people', 'People', '담당별']]),
    groups: pairs([['structure', 'Structure', '구조'], ['none', 'None', '없음'], ['status', 'Status', '칸'], ['assignee', 'Assignee', '담당'], ['priority', 'Priority', '우선순위'], ['tag', 'Tag', '태그'], ['kind', 'Kind', '종류'], ['milestone', 'Milestone', '마일스톤'], ['epic', 'Epic', '에픽']]),
    levels: pairs([['milestone', 'Milestone', '마일스톤'], ['epic', 'Epic', '에픽'], ['item', 'Item', '일']]),
    fields: SORT_KEYS.filter((key) => key !== 'id').map((id) => ({ id, label: names[id], dir: defaultDir(id) })), activeLevels,
    labels: { presets: label('Preset', '프리셋'), groups: label('Group', '묶기'), order: label('Order', '차례'), add: label('Add field', '필드 더하기'), direction: label('Reverse', '방향 뒤집기'), up: label('Move up', '위로'), down: label('Move down', '아래로'), remove: label('Remove', '빼기'), command: label('Command', '명령'), speech: label('Say', '말') },
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
  const expression = levels.map((level) => `${level}=${(panel.order[level] ?? []).map((field) => `${field.key}:${field.dir}`).join(',')}`).join(';')
  const presetName = words.presets.find((x) => x.id === panel.preset)?.label
  return { summary: presetName ?? `${words.labels.groups}: ${group} · ${pieces.join(' · ')}`,
    command: `moai ctl sort '${panel.preset ?? expression}'\nmoai ctl group ${panel.group}`,
    speech: `${group} · ${pieces.join(' · ')}` }
}
