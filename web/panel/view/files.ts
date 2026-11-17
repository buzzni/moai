import { filesSource, type Entry, type FileText, type SearchResult } from '../data/files.ts'
import { el } from './components.ts'
import { createSplit } from './split.ts'
import type { PaneView, ViewContext } from './types.ts'

const CLASSES = new Set(['comment', 'string', 'number', 'keyword', 'type', 'function', 'constant', 'tag', 'attribute', 'heading', 'other'])
const join = (parent: string, name: string) => parent === '.' ? name : `${parent}/${name}`
const fileClass = (entry: Entry): string => {
  if (entry.kind === 'dir') return 'directory'
  if (entry.kind === 'link') return 'link'
  const extension = entry.name.split('.').pop()?.toLowerCase() ?? ''
  if (['rs', 'js', 'ts', 'tsx', 'jsx', 'py', 'go', 'c', 'h'].includes(extension)) return 'source'
  if (['md', 'txt', 'rst'].includes(extension)) return 'document'
  if (['json', 'toml', 'yaml', 'yml', 'ini'].includes(extension)) return 'config'
  return 'other'
}

export function mountFiles(root: HTMLElement, ctx: ViewContext): PaneView {
  const doc = ctx.document
  const words = ctx.copy.files
  const source = ctx.client ? filesSource(ctx.client) : null
  const split = createSplit(doc, { position: 'right', ratios: { right: 0.5 }, onRatio: (position, ratio) => { if (position === 'right') ctx.intents.dispatch({ type: 'files-ratio', ratio }) } })
  const tree = el(doc, 'div', { class: 'm-files-tree', role: 'tree', tabindex: 0 })
  const content = el(doc, 'div', { class: 'm-files-content', tabindex: 0 })
  const search = el(doc, 'input', { class: 'm-files-search', type: 'search', hidden: true, 'aria-label': words.search })
  const message = el(doc, 'div', { class: 'm-files-message', role: 'status' })
  split.list.append(search, message, tree); split.detail.append(content); root.append(split.el)
  const directories = new Map<string, Entry[]>()
  const expanded = new Set<string>(['.'])
  const partial = new Set<string>()
  let beforeSearch = new Set<string>()
  let matches: SearchResult | null = null
  let cursor = 0, chosen = '', base = '.', generation = 0, pendingG = false, timer: ReturnType<typeof setTimeout> | undefined
  let rows: Array<{ path: string; entry: Entry; depth: number }> = []
  let alive = true, started = false
  async function load(dir: string, limit?: number): Promise<number> {
    if (!source) return 0
    if (directories.has(dir) && (limit !== undefined || !partial.has(dir))) return 0
    const result = await source.list(dir, limit)
    directories.set(dir, result.entries)
    if (result.truncated) partial.add(dir); else partial.delete(dir)
    return result.entries.length
  }
  function flatten(dir: string, depth: number): void {
    for (const entry of directories.get(dir) ?? []) {
      const path = join(dir, entry.name); rows.push({ path, entry, depth })
      if (expanded.has(path)) flatten(path, depth + 1)
    }
  }
  function draw(): void {
    rows = []; flatten(base, 0); cursor = Math.max(0, Math.min(cursor, rows.length - 1))
    const nodes = rows.map((row, index) => {
      const isDir = row.entry.kind === 'dir' || (row.entry.kind === 'link' && row.entry.openable && row.entry.dir === true)
      const button = el(doc, 'button', { type: 'button', role: 'treeitem', class: `m-files-row m-files-kind-${fileClass(row.entry)} m-files-depth-${Math.min(row.depth, 12)}`, 'aria-selected': String(index === cursor), ...(isDir ? { 'aria-expanded': String(expanded.has(row.path)) } : {}) }, `${isDir ? (expanded.has(row.path) ? '▾' : '▸') : row.entry.kind === 'link' ? '↗' : '·'} ${row.entry.name}`)
      const hit = matches?.matches.find(m => m.path === row.path)
      const offset = row.path.length - row.entry.name.length
      if (hit && hit.start >= offset && hit.end <= row.path.length && hit.end >= hit.start) {
        const prefix = isDir ? (expanded.has(row.path) ? '▾' : '▸') : row.entry.kind === 'link' ? '↗' : '·'
        button.replaceChildren(doc.createTextNode(`${prefix} ${row.entry.name.slice(0, hit.start - offset)}`), el(doc, 'mark', {}, row.entry.name.slice(hit.start - offset, hit.end - offset)), doc.createTextNode(row.entry.name.slice(hit.end - offset)))
      }
      if (!row.entry.openable) button.append(doc.createTextNode(` (${words.unavailable})`))
      button.disabled = !row.entry.openable
      button.addEventListener('click', () => { cursor = index; if (isDir) { generation++; clearTimeout(timer) } if (isDir) { if (expanded.has(row.path)) { expanded.delete(row.path); draw() } else void expand() } else { select(); draw() } })
      button.addEventListener('dblclick', () => { void expand() })
      return button
    })
    if (base !== '.') {
      const up = el(doc, 'button', { type: 'button', role: 'treeitem', class: 'm-files-row' }, '↰ ..')
      up.addEventListener('click', () => { base = base.includes('/') ? base.slice(0, base.lastIndexOf('/')) : '.'; cursor = 0; draw() })
      nodes.unshift(up)
    }
    tree.replaceChildren(...nodes)
  }
  function paintFile(file: FileText): void {
    content.replaceChildren()
    if (file.binary) { content.textContent = words.binary; return }
    const lines = (file.text ?? '').split('\n'); let index = 0
    const drawChunk = () => {
      if (!alive || chosen !== file.path) return
      const fragment = doc.createDocumentFragment()
      for (let end = Math.min(index + 200, lines.length); index < end; index++) {
        const text = lines[index]!, row = el(doc, 'div', { class: 'm-file-line' }), number = el(doc, 'span', { class: 'm-file-number' }, String(index + 1)), code = el(doc, 'span', { class: 'm-file-code' })
        let at = 0
        for (const span of file.spans?.[index] ?? []) {
          if (!Array.isArray(span) || !Number.isInteger(span[0]) || !Number.isInteger(span[1]) || span[0] < at || span[1] < span[0] || span[1] > text.length) continue
          code.append(doc.createTextNode(text.slice(at, span[0])))
          const kind = CLASSES.has(span[2]) ? span[2] : 'other'
          code.append(el(doc, 'span', { class: `m-syntax-${kind}` }, text.slice(span[0], span[1]))); at = span[1]
        }
        code.append(doc.createTextNode(text.slice(at))); row.append(number, code); fragment.append(row)
      }
      content.append(fragment)
      if (index < lines.length) setTimeout(drawChunk, 0)
      else if (file.truncated) content.append(el(doc, 'p', {}, words.truncated))
    }; drawChunk()
  }
  function select(): void {
    const row = rows[cursor]; const current = ++generation; clearTimeout(timer)
    if (!row || !(row.entry.kind === 'file' || (row.entry.kind === 'link' && !row.entry.dir && row.entry.openable)) || !source) return
    chosen = row.path
    timer = setTimeout(() => { void source.file(row.path).then(file => { if (alive && generation === current) paintFile(file) }).catch(() => { if (generation === current) content.textContent = words.readError }) }, 100)
  }
  async function expand(): Promise<void> {
    const row = rows[cursor]; if (!row || !(row.entry.kind === 'dir' || (row.entry.kind === 'link' && row.entry.dir && row.entry.openable))) return
    if (expanded.has(row.path)) return
    try { await load(row.path); expanded.add(row.path); draw() } catch { message.textContent = words.directoryError }
  }
  async function expandAll(): Promise<void> {
    if (!source) return
    if (rows.some(row => row.entry.kind === 'dir' && expanded.has(row.path))) {
      for (const row of rows) expanded.delete(row.path)
      draw(); return
    }
    const ignored = await source.ignored().catch(() => [])
    const pending = [base]; let count = 0
    while (pending.length && count < 5000) {
      const dir = pending.pop()!
      try { count += await load(dir, 5000 - count) } catch { continue }
      expanded.add(dir)
      const entries = directories.get(dir) ?? []
      for (const entry of entries) {
        const path = join(dir, entry.name)
        if (entry.kind === 'dir' && !ignored.some(item => path === item.replace(/\/$/, '') || path.startsWith(`${item.replace(/\/$/, '')}/`))) pending.push(path)
      }
    }
    if (pending.length || partial.size) message.textContent = words.expansionLimit
    draw()
  }
  async function find(q: string): Promise<void> {
    if (!source || !q) return
    const current = ++generation
    try {
      const result = await source.search(q, base); if (current !== generation) return
      matches = result
      for (const hit of result.matches) {
        const parts = hit.path.split('/'); parts.pop(); let dir = '.'
        for (const part of parts) { dir = join(dir, part); await load(dir); if (current !== generation || !alive) return; expanded.add(dir) }
      }
      message.textContent = `${result.total} ${words.matches}${result.truncated || result.walk_truncated ? ` (${words.truncated})` : ''}${result.ignored_unavailable ? ` (${words.ignoredUnavailable})` : ''}`; draw()
    } catch { message.textContent = words.searchError }
  }
  search.addEventListener('input', () => {
    clearTimeout(timer); generation++
    if (!search.value) { generation++; matches = null; expanded.clear(); for (const p of beforeSearch) expanded.add(p); message.textContent = ''; draw(); return }
    timer = setTimeout(() => void find(search.value), 200)
  })
  return {
    render(state) { split.setWidth(root.clientWidth); split.setRatio('right', state.filesRatio); if (!started) { started = true; void load('.').then(() => { if (alive) draw() }).catch(() => { message.textContent = words.directoryError }) } }, focus() { tree.focus() },
    handleKey(event) {
      if (event.target === search) {
        if (event.key === 'Escape') { search.hidden = true; search.value = ''; expanded.clear(); for (const p of beforeSearch) expanded.add(p); matches = null; message.textContent = ''; draw(); tree.focus(); return true }
        if (event.key === 'Enter') { search.hidden = true; tree.focus(); return true }
        return false
      }
      const row = rows[cursor]
      if (event.key === 'Tab') { void expandAll(); return true }
      if (event.key === 'g') { if (pendingG) { cursor = 0; pendingG = false; select(); draw() } else pendingG = true; return true }
      pendingG = false
      if (event.key === 'H' || event.key === 'L') { content.scrollLeft += event.key === 'H' ? -80 : 80; return true }
      if (event.ctrlKey && (event.key === 'd' || event.key === 'u')) { cursor = Math.max(0, Math.min(rows.length - 1, cursor + (event.key === 'd' ? 10 : -10))); select(); draw(); return true }
      if (event.key === '/') { if (!matches) beforeSearch = new Set(expanded); search.hidden = false; search.focus(); return true }
      if (event.key === 'Escape' && matches) { search.value = ''; expanded.clear(); for (const p of beforeSearch) expanded.add(p); matches = null; message.textContent = ''; generation++; draw(); return true }
      if (event.key === 'j' || event.key === 'ArrowDown') cursor = Math.min(rows.length - 1, cursor + 1)
      else if (event.key === 'k' || event.key === 'ArrowUp') cursor = Math.max(0, cursor - 1)
      else if (event.key === 'G') cursor = rows.length - 1
      else if (event.key === 'l' || event.key === 'ArrowRight') { void expand(); return true }
      else if (event.key === 'h' || event.key === 'ArrowLeft') { if (row && expanded.has(row.path)) expanded.delete(row.path); else if (row?.path.includes('/')) { const parent = row.path.slice(0, row.path.lastIndexOf('/')); const i = rows.findIndex(r => r.path === parent); if (i >= 0) cursor = i } }
      else if (event.key === 'Enter') { if (row && (row.entry.kind === 'dir' || (row.entry.kind === 'link' && row.entry.dir && row.entry.openable))) { base = row.path; expanded.add(base); void load(base).then(draw) } else { root.classList.add('m-files-show-content'); content.focus() } return true }
      else if (event.key === 'Backspace') { if (root.classList.contains('m-files-show-content')) { root.classList.remove('m-files-show-content'); tree.focus(); return true } if (base !== '.') { base = base.includes('/') ? base.slice(0, base.lastIndexOf('/')) : '.'; cursor = 0 } }
      else if ((event.key === 'n' || event.key === 'N') && matches?.matches.length) { const found = rows.map((r, i) => matches?.matches.some(m => m.path === r.path) ? i : -1).filter(i => i >= 0); if (found.length) cursor = event.key === 'n' ? found.find(i => i > cursor) ?? found[0]! : [...found].reverse().find(i => i < cursor) ?? found[found.length - 1]! }
      else return false
      select(); draw(); return true
    }, command(action) {
      if (!action.startsWith('focus.')) return false
      const side = action.slice(6)
      const show = side === 'right' || ((side === 'next' || side === 'prev') && doc.activeElement !== content)
      if (show) { root.classList.add('m-files-show-content'); content.focus() } else { root.classList.remove('m-files-show-content'); tree.focus() }
      return true
    }, destroy() { alive = false; clearTimeout(timer); split.destroy() },
  }
}
