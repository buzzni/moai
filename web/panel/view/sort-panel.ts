/** Sort and group controls. The caller supplies the server vocabulary and owns the state. */
export interface SortPanelField { key: string; dir: 'asc' | 'desc' }
export interface SortPanelState {
  group: string
  order: Record<string, SortPanelField[]>
  level: string
  preset?: string
  edited?: boolean
  command?: string
  speech?: string
}
export interface SortPanelVocabulary {
  presets: readonly { id: string; label: string }[]
  groups: readonly { id: string; label: string }[]
  levels: readonly { id: string; label: string }[]
  fields: readonly { id: string; label: string; dir: 'asc' | 'desc'; levels?: readonly string[] }[]
  activeLevels: (group: string) => readonly string[]
  labels: { presets: string; groups: string; order: string; add: string; direction: string; up: string; down: string; remove: string; command?: string; speech?: string }
}
export type SortPanelAction =
  | { type: 'preset'; id: string }
  | { type: 'group'; id: string }
  | { type: 'level'; id: string }
  | { type: 'fields'; level: string; fields: SortPanelField[] }

/** Build from the current state; caller redraws after each action. All labels are injected. */
export function sortPanel(doc: Document, state: SortPanelState, words: SortPanelVocabulary, send: (action: SortPanelAction) => void): HTMLElement {
  const root = doc.createElement('div')
  root.className = 'm-sort-panel'
  const section = (label: string): HTMLElement => {
    const node = doc.createElement('section')
    const title = doc.createElement('div')
    title.className = 'm-sort-section-label'
    title.textContent = label
    node.append(title)
    root.append(node)
    return node
  }
  let focusId = 0
  const button = (label: string, action: () => void): HTMLButtonElement => {
    const node = doc.createElement('button')
    node.type = 'button'
    node.dataset.sortFocus = `button-${focusId++}`
    node.textContent = label
    node.addEventListener('click', action)
    return node
  }
  const presets = section(words.labels.presets)
  for (const preset of words.presets) {
    const node = button(preset.label, () => send({ type: 'preset', id: preset.id }))
    node.setAttribute('aria-pressed', String(preset.id === state.preset))
    presets.append(node)
  }
  const groups = section(words.labels.groups)
  for (const group of words.groups) {
    const node = button(group.label, () => send({ type: 'group', id: group.id }))
    node.setAttribute('aria-pressed', String(group.id === state.group))
    groups.append(node)
  }
  const order = section(words.labels.order)
  const tabs = doc.createElement('div')
  tabs.setAttribute('role', 'tablist')
  const active = words.activeLevels(state.group)
  for (const level of words.levels) {
    const node = button(level.label, () => send({ type: 'level', id: level.id }))
    node.setAttribute('role', 'tab')
    node.setAttribute('aria-selected', String(level.id === state.level))
    node.disabled = !active.includes(level.id)
    tabs.append(node)
  }
  order.append(tabs)
  const fields = state.order[state.level] ?? []
  const change = (next: SortPanelField[]): void => send({ type: 'fields', level: state.level, fields: next })
  const list = doc.createElement('ol')
  fields.forEach((field, index) => {
    const row = doc.createElement('li')
    row.dataset.key = field.key
    row.className = 'm-menu-item'
    const name = doc.createElement('span')
    name.textContent = words.fields.find((one) => one.id === field.key)?.label ?? field.key
    row.append(name)
    const flip = button(field.dir === 'asc' ? '↑' : '↓', () => change(fields.map((one, n) => n === index ? { ...one, dir: one.dir === 'asc' ? 'desc' : 'asc' } : one)))
    flip.setAttribute('aria-label', words.labels.direction)
    row.append(flip)
    for (const [delta, label] of [[-1, words.labels.up], [1, words.labels.down]] as const) {
      const move = button(delta < 0 ? '▲' : '▼', () => {
        const next = fields.slice()
        const target = index + delta
        if (target < 0 || target >= next.length) return
        ;[next[index], next[target]] = [next[target]!, next[index]!]
        change(next)
      })
      move.disabled = index + delta < 0 || index + delta >= fields.length
      move.setAttribute('aria-label', label)
      row.append(move)
    }
    const remove = button('×', () => change(fields.filter((_, n) => n !== index)))
    remove.disabled = fields.length <= 1
    remove.setAttribute('aria-label', words.labels.remove)
    row.append(remove)
    list.append(row)
  })
  for (const field of words.fields.filter((one) => !fields.some((f) => f.key === one.id) && (!one.levels || one.levels.includes(state.level)))) {
    const row = doc.createElement('li')
    row.dataset.key = field.id
    row.className = 'm-menu-item m-sort-unused'
    row.textContent = `· ${field.label}`
    list.append(row)
  }
  order.append(list)
  const select = doc.createElement('select')
  select.setAttribute('aria-label', words.labels.add)
  select.dataset.sortFocus = 'select'
  for (const field of words.fields.filter((one) => !fields.some((f) => f.key === one.id) && (!one.levels || one.levels.includes(state.level)))) {
    const option = doc.createElement('option')
    option.value = field.id
    option.textContent = field.label
    select.append(option)
  }
  const add = button(words.labels.add, () => {
    const field = words.fields.find((one) => one.id === select.value)
    if (field) change([...fields, { key: field.id, dir: field.dir }])
  })
  add.disabled = select.options.length === 0
  order.append(select, add)
  if (state.command && words.labels.command) {
    const command = section(words.labels.command)
    const code = doc.createElement('pre')
    code.textContent = state.command
    command.append(code)
  }
  if (state.speech && words.labels.speech) {
    const speech = section(words.labels.speech)
    const text = doc.createElement('q')
    text.textContent = state.speech
    speech.append(text)
  }
  return root
}
