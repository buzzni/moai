import { el } from './components.ts'

export type DiffLayout = 'unified' | 'side-by-side'
export interface DiffLine { kind: 'context' | 'add' | 'delete' | 'raw'; old_no: number | null; new_no: number | null; text: string; spans?: Array<[number, number, string]> | null }
export interface DiffHunk { old_start: number; old_len: number; new_start: number; new_len: number; section: string; lines: DiffLine[] }
export interface DiffFile { path: string; old_path: string | null; status: string; adds: number | null; dels: number | null; binary: boolean; hunks?: DiffHunk[] }
export interface DiffAnswer { files: DiffFile[]; truncated: boolean; vcs_error?: { said: string } | null }
export interface DiffWords { unified: string; side: string; narrow: string; binary: string; empty: string; truncated: string }
const kinds = new Set(['comment', 'string', 'number', 'keyword', 'type', 'function', 'constant', 'tag', 'attribute', 'heading', 'other'])
export const statusMark = (status: string): string => ({ added: 'A', modified: 'M', deleted: 'D', renamed: 'R', copied: 'C', type_changed: 'T', untracked: '?', conflicted: 'U' } as Record<string, string>)[status] ?? '?'

/** Align each contiguous deletion/addition run in the same order as the shared Rust reader. */
export function pair(lines: DiffLine[]): Array<[DiffLine | null, DiffLine | null]> {
  const result: Array<[DiffLine | null, DiffLine | null]> = []
  let at = 0
  while (at < lines.length) {
    const line = lines[at]!
    if (line.kind === 'context' || line.kind === 'raw') { result.push([line, line]); at++; continue }
    const left: DiffLine[] = [], right: DiffLine[] = []
    while (at < lines.length && lines[at]!.kind !== 'context' && lines[at]!.kind !== 'raw') {
      const next = lines[at++]!
      if (next.kind === 'delete') left.push(next); else if (next.kind === 'add') right.push(next)
    }
    for (let n = 0; n < Math.max(left.length, right.length); n++) result.push([left[n] ?? null, right[n] ?? null])
  }
  return result
}
function textWithSpans(doc: Document, text: string, spans: DiffLine['spans']): HTMLElement {
  const out = el(doc, 'span', { class: 'm-diff-text' }); let pos = 0
  for (const span of spans ?? []) {
    if (!Array.isArray(span) || !Number.isInteger(span[0]) || !Number.isInteger(span[1]) || span[0] < pos || span[1] < span[0] || span[1] > text.length) continue
    out.append(doc.createTextNode(text.slice(pos, span[0])))
    out.append(el(doc, 'span', { class: `m-syntax-${kinds.has(span[2]) ? span[2] : 'other'}` }, text.slice(span[0], span[1])))
    pos = span[1]
  }
  out.append(doc.createTextNode(text.slice(pos))); return out
}
function lineElement(doc: Document, line: DiffLine | null, side?: 'old' | 'new'): HTMLElement {
  if (!line) return el(doc, 'div', { class: 'm-diff-line m-diff-blank' })
  const sign = line.kind === 'add' ? '+' : line.kind === 'delete' ? '-' : ' '
  const number = side === 'old' ? line.old_no : side === 'new' ? line.new_no : null
  const kind = ['context', 'add', 'delete', 'raw'].includes(line.kind) ? line.kind : 'raw'
  return el(doc, 'div', { class: `m-diff-line m-diff-${kind}` },
    el(doc, 'span', { class: 'm-diff-no' }, side ? String(number ?? '') : String(line.old_no ?? '')),
    ...(side ? [] : [el(doc, 'span', { class: 'm-diff-no' }, String(line.new_no ?? ''))]),
    el(doc, 'span', { class: 'm-diff-sign' }, sign), textWithSpans(doc, line.text, line.spans))
}
export function createDiff(doc: Document, words: DiffWords, onLayout?: (layout: DiffLayout) => void) {
  const root = el(doc, 'section', { class: 'm-diff', tabindex: 0 })
  const toolbar = el(doc, 'div', { class: 'm-diff-toolbar' })
  const toggle = el(doc, 'button', { type: 'button' })
  const notice = el(doc, 'span', { role: 'status' })
  const body = el(doc, 'div', { class: 'm-diff-body' })
  toolbar.append(toggle, notice); root.append(toolbar, body)
  let layout: DiffLayout = 'unified', answer: DiffAnswer = { files: [], truncated: false }, width = Infinity
  const open = new Set<string>()
  let defaultOpen = true, revision = 0, restoreScroll: { top: number; left: number } | null = null
  function paint(): void {
    const version = ++revision
    const side = layout === 'side-by-side' && width >= 640
    toggle.textContent = layout === 'side-by-side' ? words.unified : words.side
    notice.textContent = layout === 'side-by-side' && !side ? words.narrow : ''
    body.replaceChildren()
    if (answer.vcs_error) { body.textContent = answer.vcs_error.said; return }
    if (!answer.files.length) { body.textContent = words.empty; return }
    answer.files.forEach((file, index) => {
      const details = el(doc, 'details', { class: 'm-diff-file', 'data-path': file.path })
      details.open = open.has(file.path) || (defaultOpen && index === 0 && open.size === 0)
      const name = file.old_path && file.old_path !== file.path ? `${file.old_path} → ${file.path}` : file.path
      details.append(el(doc, 'summary', {}, `${statusMark(file.status)} ${name}  +${file.adds ?? 0} −${file.dels ?? 0}`))
      let built = false
      const build = () => {
        if (built) return; built = true
        if (file.binary || !file.hunks?.length) { details.append(el(doc, 'p', {}, file.binary ? words.binary : words.empty)); return }
        const tasks: Array<() => Node[]> = []
        for (const hunk of file.hunks) {
          tasks.push(() => [el(doc, 'div', { class: 'm-diff-hunk' }, `@@ -${hunk.old_start},${hunk.old_len} +${hunk.new_start},${hunk.new_len} @@ ${hunk.section}`)])
          if (side) {
            const grid = el(doc, 'div', { class: 'm-diff-pairs' })
            tasks.push(() => [grid])
            for (const [old, next] of pair(hunk.lines)) tasks.push(() => { grid.append(lineElement(doc, old, 'old'), lineElement(doc, next, 'new')); return [] })
          } else for (const line of hunk.lines) tasks.push(() => [lineElement(doc, line)])
        }
        let at = 0
        const chunk = () => {
          if (version !== revision) return
          const fragment = doc.createDocumentFragment()
          for (const end = Math.min(at + 200, tasks.length); at < end; at++) fragment.append(...tasks[at]!())
          details.append(fragment)
          if (restoreScroll) { body.scrollTop = restoreScroll.top; body.scrollLeft = restoreScroll.left }
          if (at < tasks.length) setTimeout(chunk, 0)
        }
        chunk()
      }
      details.addEventListener('toggle', () => { if (details.open) { open.add(file.path); build() } else open.delete(file.path); root.dispatchEvent(new doc.defaultView!.Event('moai-file-watch', { bubbles: true })) })
      if (details.open) build()
      body.append(details)
    })
    if (answer.truncated) body.append(el(doc, 'p', {}, words.truncated))
  }
  toggle.addEventListener('click', () => { layout = layout === 'unified' ? 'side-by-side' : 'unified'; onLayout?.(layout); paint() })
  return {
    el: root, show(value: DiffAnswer, preserve = false) {
      const top = body.scrollTop, left = body.scrollLeft
      if (preserve) for (const file of body.querySelectorAll<HTMLDetailsElement>('details')) {
        const path = file.dataset.path; if (path) { if (file.open) open.add(path); else open.delete(path) }
      } else open.clear()
      restoreScroll = preserve ? { top, left } : null
      defaultOpen = !preserve; answer = value; paint(); if (preserve) { body.scrollTop = top; body.scrollLeft = left }
    },
    setLayout(value: DiffLayout) { if (layout !== value) { layout = value; paint() } },
    setWidth(value: number) { const changed = (width >= 640) !== (value >= 640); width = value || Infinity; if (changed) paint() },
    focus() { root.focus() },
    visiblePath(): string | undefined { return body.querySelector<HTMLDetailsElement>('details[open]')?.dataset.path },
    command(action: string): boolean {
      if (action === 'toggle') { toggle.click(); return true }
      if (action === 'left' || action === 'right') { body.scrollLeft += action === 'left' ? -80 : 80; return true }
      if (action === 'next' || action === 'prev') { const files = [...body.querySelectorAll<HTMLElement>('summary')]; const at = files.indexOf(doc.activeElement as HTMLElement); files[(at + (action === 'next' ? 1 : files.length - 1)) % files.length]?.focus(); return files.length > 0 }
      return false
    },
  }
}
