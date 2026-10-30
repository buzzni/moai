/** The visible tab's local control interpreter. It uses the same store and key handlers as a person. */
import { defaultDir, SORT_KEYS, type SortField } from './data/query.js'
import { sortDefault } from './data/memory.js'
import { isUnread, type ReadMarks } from './data/read.js'
import { validZone } from './data/zone.js'
import type { KeyWiring } from './keys/wiring.js'
import { bindingsFor, bindingKeys, isAvailable } from './keys/table.js'
import { detectPlatform } from './keys/router.js'
import type { Store, Tab, ColumnName } from './store.js'
import { clampRatio, DEFAULT_RATIO } from './view/split.js'
import { DEFAULT_GRID } from './view/gantt-model.js'
import type { Shell } from './view/shell.js'

export interface Request { verb: string; [key: string]: unknown }
export interface Reply { ok: boolean; code?: string; message?: string; data?: unknown }
export const yes = (data: unknown = {}): Reply => ({ ok: true, data })
export const no = (code: string, message: string): Reply => ({ ok: false, code, message })

export interface CtlOptions {
  store: Store
  keys: KeyWiring
  shell: Shell
  read: ReadMarks
  drawnOrder(): string[]
  openProject?(path: string): boolean
  wikiSearch?(query: string): Promise<{ slug: string; hits: unknown[] } | null>
  /** Captures the identity synchronously when a read-mark write starts. */
  withIdentity<T>(seq: number, token: string, work: () => T): T
}

export function createCtl(options: CtlOptions) {
  const { store, keys, shell, read } = options
  const doc = shell.el.ownerDocument
  let ownedInput: Element | null = null
  let ownedMenu = false
  doc.addEventListener('keydown', (event) => {
    if (event.isTrusted) { ownedInput = null; ownedMenu = false }
  }, true)
  doc.addEventListener('pointerdown', (event) => {
    if (event.isTrusted) { ownedInput = null; ownedMenu = false }
  }, true)
  const current = () => store.getState()
  const busy = (): boolean => {
    const active = doc.activeElement
    if (active && active !== doc.body && active.matches('input, textarea, select, [contenteditable="true"]') && active !== ownedInput) return true
    return keys.router.state().menu !== null && !ownedMenu
  }
  const screen = (name: unknown): name is Tab => name === 'board' || name === 'list' || name === 'gantt' || name === 'wiki'
  const after = (): Reply => {
    const state = current()
    return yes({ screen: state.tab, cursor: state.cursor, mode: keys.context().mode, menu: keys.router.state().menu, changed: true })
  }
  const state = (rows: boolean): Reply => {
    const s = current()
    const shown = options.drawnOrder().slice(0, 2000)
    const index = new Map((s.board.value?.rows ?? []).map((row) => [row.id, row]))
    return yes({
      screen: s.tab, path: s.path, cursor: s.cursor, focus: s.focus, mode: keys.context().mode,
      menu: keys.router.state().menu,
      filter: s.filter.applied ? { text: s.filter.applied, kind: 'filter', scope: 'all' } : s.search.text ? { text: s.search.text, kind: 'grep', scope: 'all' } : null,
      sort: s.sort.fields.map((field) => field.dir === defaultDir(field.key) ? field.key : `${field.key}:${field.dir}`),
      view: { hidden: s.view.hiddenStatuses, deferred: !s.view.hideDeferred, backlog: !s.view.hideBacklog,
        archived: s.view.showArchived, worktree: s.view.worktree },
      cells: Object.entries(s.columns).filter(([, on]) => on).map(([name]) => name),
      detail: { shown: s.detail.visible, at: s.detail.position },
      panes: { list_width: Math.round((1 - s.detail.ratio[s.detail.position]) * 100),
        list_height: Math.round((1 - s.detail.ratio[s.detail.position]) * 100),
        gantt_grid: shell.panes().gantt.gridWidth?.() ?? null },
      theme: s.theme, timezone: s.zone, wiki: s.tab === 'wiki' ? s.wiki : null,
      unread: (s.board.value?.rows ?? []).filter(isUnread).length,
      proposals: [],
      ...(rows ? { rows: shown.map((id) => { const row = index.get(id); return { id, status: row?.status, title: row?.title } }) } : {}),
    })
  }
  const actions = (): Reply => {
    const context = keys.context()
    const catalog = bindingsFor(context.mode)
    const platform = detectPlatform(doc.defaultView?.navigator)
    return yes({ actions: keys.actions.map((name) => {
      const bindings = catalog.filter((binding) => binding.action === name)
      return { name, keys: bindings.flatMap((binding) => bindingKeys(binding, platform)), enabled: bindings.some((binding) => isAvailable(binding, context)) }
    }) })
  }
  const press = (seqs: unknown, text: unknown): Reply => {
    if (!Array.isArray(seqs) || seqs.some((seq) => typeof seq !== 'string')) return no('bad_input', 'key sequences must be strings')
    if (text !== null && text !== undefined && typeof text !== 'string') return no('bad_input', 'text must be a string')
    for (const seq of seqs as string[]) {
      for (const token of seq.split(/\s+/).filter(Boolean)) {
        if (!keys.router.press(token).handled) return no('bad_input', `unknown key: ${token}`)
      }
    }
    ownedMenu = keys.router.state().menu !== null
    const active = doc.activeElement
    if (active?.matches('input, textarea')) ownedInput = active
    if (typeof text === 'string') {
      if (!ownedInput || active !== ownedInput || !(active instanceof (doc.defaultView?.HTMLInputElement ?? HTMLInputElement))) {
        return no('person_busy', 'text needs a search or filter opened by ctl')
      }
      active.value += text
      active.dispatchEvent(new (doc.defaultView?.Event ?? Event)('input', { bubbles: true }))
    }
    return after()
  }
  const setPane = (request: Request): Reply => {
    if (request.wiki_width != null || request.gantt_label != null) return no('unsupported', 'that pane unit is unavailable on the web')
    const widths = [request.list_width, request.list_height].filter((value) => value !== null && value !== undefined)
    if (widths.length && (widths.length > 1 || typeof widths[0] !== 'string' || (widths[0] !== 'none' && !/^\d+$/.test(widths[0])))) return no('bad_input', 'give one numeric pane share')
    if (widths.length) {
      const s = current()
      const position = s.detail.position
      if (request.list_height != null && position !== 'top' && position !== 'bottom') return no('unsupported', 'list height needs a top or bottom detail')
      if (request.list_width != null && position !== 'left' && position !== 'right') return no('unsupported', 'list width needs a side detail')
      const total = shell.layout().width
      const ratio = widths[0] === 'none' ? DEFAULT_RATIO[position] : clampRatio(1 - Number(widths[0]) / 100, total, position)
      store.dispatch({ type: 'detail-ratio', ratio, position })
    }
    if (request.gantt_grid != null) {
      if (typeof request.gantt_grid !== 'string' || (request.gantt_grid !== 'none' && !/^\d+$/.test(request.gantt_grid))) return no('bad_input', 'gantt grid needs pixels')
      shell.panes().gantt.setGrid?.(request.gantt_grid === 'none' ? DEFAULT_GRID : Number(request.gantt_grid))
    }
    return after()
  }
  const mark = async (request: Request, seq: number, token: string): Promise<Reply> => {
    const ids = request.ids
    let result
    if (request.all === true) result = await options.withIdentity(seq, token, () => read.all())
    else if (typeof request.group === 'string') result = await options.withIdentity(seq, token, () => read.group(String(request.group)))
    else if (Array.isArray(ids) && ids.every((id) => typeof id === 'string')) {
      for (const id of ids as string[]) {
        result = await options.withIdentity(seq, token, () => read.one(id))
        if (result) break
      }
    } else return no('bad_input', 'give ids, all or group')
    return result ? no(result.kind === 'moai-error' ? result.code ?? 'refused' : 'refused', 'message' in result ? String(result.message) : 'read was refused') : after()
  }

  async function run(request: Request, seq: number, token: string): Promise<Reply> {
    if (request.verb === 'state') return state(request.rows === true)
    if (request.verb === 'actions') return actions()
    if (busy()) return no('person_busy', 'the person is using an input or menu')
    try {
      return await options.withIdentity(seq, token, () => execute(request, seq, token))
    } catch (error) {
      return no('refused', error instanceof Error ? error.message : String(error))
    }
  }

  async function execute(request: Request, seq: number, token: string): Promise<Reply> {
    const s = current()
    switch (request.verb) {
      case 'keys': return press(request.seqs, request.text)
      case 'do':
        return typeof request.name === 'string' && keys.run(request.name) ? after() : no('unsupported', 'unknown web action')
      case 'screen':
        if (!screen(request.name)) return no('unsupported', 'this screen is unavailable on the web')
        store.dispatch({ type: 'tab', tab: request.name }); return after()
      case 'sort': {
        if (!Array.isArray(request.words) || request.words.some((word) => typeof word !== 'string')) return no('bad_input', 'invalid sort words')
        const fields: SortField[] = []
        for (const word of request.words as string[]) {
          const [key, direction] = word.split(':')
          if (!SORT_KEYS.includes(key as SortField['key']) || key === 'id' || (direction && direction !== 'asc' && direction !== 'desc')) return no('bad_input', 'invalid sort field')
          fields.push({ key: key as SortField['key'], dir: (direction ?? defaultDir(key as SortField['key'])) as SortField['dir'] })
        }
        const sort = { fields, reversed: false }
        store.dispatch({ type: 'sort', sort })
        if (request.default === true) {
          sortDefault.set(sort)
          store.dispatch({ type: 'sort-default', sort })
        }
        return after()
      }
      case 'filter':
        if (request.query !== null && typeof request.query !== 'string') return no('bad_input', 'filter needs text')
        store.dispatch({ type: 'filter', text: request.query ?? '' }); return after()
      case 'find':
        if (request.scope !== 'all') return no('unsupported', 'web search covers all fields')
        if (typeof request.query !== 'string') return no('bad_input', 'search needs text')
        store.dispatch({ type: 'search', text: request.query }); return after()
      case 'view': {
        if (typeof request.on !== 'boolean') return no('bad_input', 'view needs on or off')
        const view = s.view
        const name = request.name
        if (name === 'deferred') store.dispatch({ type: 'view', view: { hideDeferred: !request.on } })
        else if (name === 'backlog') store.dispatch({ type: 'view', view: { hideBacklog: !request.on } })
        else if (name === 'archived') store.dispatch({ type: 'view', view: { showArchived: request.on } })
        else if (name === 'worktree') store.dispatch({ type: 'view', view: { worktree: request.on } })
        else if (name === 'raw') {
          const pane = s.tab === 'wiki' ? shell.panes().wiki : shell.panes().detail
          if (pane?.raw?.() !== request.on) pane?.toggleRaw?.()
        }
        else if (typeof name === 'string' && name.startsWith('column:') && s.boot?.statuses.includes(name.slice(7))) {
          const column = name.slice(7)
          store.dispatch({ type: 'view', view: { hiddenStatuses: request.on ? view.hiddenStatuses.filter((one) => one !== column) : [...new Set([...view.hiddenStatuses, column])] } })
        }
        else return no('bad_input', 'unknown view')
        return after()
      }
      case 'cell':
        if (typeof request.name !== 'string' || !(request.name in s.columns) || typeof request.on !== 'boolean') return no('bad_input', 'unknown cell')
        store.dispatch({ type: 'column', column: request.name as ColumnName, on: request.on }); return after()
      case 'detail':
        if (request.name === 'on' || request.name === 'off') store.dispatch({ type: 'detail-visible', visible: request.name === 'on' })
        else if (request.name === 'right' || request.name === 'left' || request.name === 'top' || request.name === 'bottom') store.dispatch({ type: 'detail-position', position: request.name })
        else return no('bad_input', 'unknown detail position')
        return after()
      case 'goto':
        if (typeof request.project === 'string') {
          const project = s.projects.find((entry) => entry.path === request.project || String(entry.number) === request.project)
          return project && options.openProject?.(project.path) ? after() : no('not_found', 'project is not open')
        }
        if (typeof request.id !== 'string' || !s.board.value?.rows.some((row) => row.id === request.id)) return no('not_found', 'row is not on screen')
        store.dispatch({ type: 'cursor', id: request.id });
        if (request.enter === true) keys.run('enter')
        return after()
      case 'read': return mark(request, seq, token)
      case 'wiki': {
        let slug = request.slug
        let hits: unknown[] | null = null
        if (typeof request.search === 'string') {
          const found = await options.wikiSearch?.(request.search) ?? null
          slug = found?.slug ?? null
          hits = found?.hits ?? []
        }
        if (typeof slug !== 'string' || !slug) return no('not_found', 'wiki page not found')
        store.dispatch({ type: 'tab', tab: 'wiki' })
        const [page, anchor] = slug.split('#', 2)
        if (shell.panes().wiki?.openWiki) await shell.panes().wiki!.openWiki!(page!, anchor)
        else store.dispatch({ type: 'wiki', wiki: { ...current().wiki, current: page!, query: '' } })
        const reply = after()
        return hits ? { ...reply, data: { ...(reply.data as object), hits } } : reply
      }
      case 'pane': return setPane(request)
      case 'zone':
        if (request.name !== 'none' && !validZone(request.name)) return no('bad_input', 'unknown time zone')
        store.dispatch({ type: 'zone', zone: request.name === 'none' ? null : String(request.name) }); return after()
      case 'theme':
        if (request.name !== 'system' && request.name !== 'light' && request.name !== 'dark') return no('bad_input', 'unknown theme')
        store.dispatch({ type: 'theme', theme: request.name }); return after()
      default: return no('unsupported', 'this control verb is unavailable on the web')
    }
  }

  return { run, state }
}
