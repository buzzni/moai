import { el, iconButton, paintState, stateLabel } from './components.ts'
import { fill as fillWords, type Copy } from '../i18n/index.ts'

export type DiffLayout = 'unified' | 'side-by-side'
export interface DiffLine { kind: 'context' | 'add' | 'delete' | 'raw'; old_no: number | null; new_no: number | null; text: string; spans?: Array<[number, number, string]> | null }
export interface DiffHunk { old_start: number; old_len: number; new_start: number; new_len: number; section: string; lines: DiffLine[] }
export interface DiffFile { path: string; old_path: string | null; status: string; adds: number | null; dels: number | null; binary: boolean; mark?: string; hunks?: DiffHunk[] }
/** `highlight`: how far the server's colouring got (`full`, `partial`, `none`) — text and hunks are whole either way. */
export interface DiffAnswer { files: DiffFile[]; truncated: boolean; vcs_error?: { said: string } | null; highlight?: string }
/** What the pane lists before any patch: the files' heads (no hunks). */
export interface DiffList { files: DiffFile[]; truncated: boolean }
export interface DiffWords {
  unified: string; side: string; narrow: string; binary: string; empty: string; truncated: string
  /** Beside the spinner: the spinner alone says nothing with reduced motion, nor to a screen reader. */
  reading: string
  retry: string
  /** The server gave up (504) — said apart from any other failure. */
  timeout: string
  /** `{n}`: the file's changed lines. */
  large: string
  anyway: string
  /** Under a file whose answer came without colours. */
  plain: string
  /** Before the terminal line that shows a file too large to fetch. */
  terminal: string
}
const kinds = new Set(['comment', 'string', 'number', 'keyword', 'type', 'function', 'constant', 'tag', 'attribute', 'heading', 'other'])
/** Closed list: the server's letter shows, and takes its colour class, only from here; any other value leaves the chip empty. */
const MARKS = new Set(['M', 'A', 'D', 'R'])
/** How long a read may run before its spinner stands: a quick answer never flashes one. */
export const SPIN_DELAY_MS = 200
/** A file whose `+n −n` passes this is not fetched until asked: past it the highlighting stops and the page draws for long. */
export const LARGE_LINES = 5000

/**
 * One file's patch read, handed out by the pane through `want`. The pane does not know requests: whoever holds the
 * ticket reads, then calls `done` or `failed`. A ticket the pane moved past (another issue, scope, checkout, a newer
 * read of the file) is dead — `live()` says so, and `done`/`failed` on it do nothing, so a late answer never lands
 * and a dropped read leaves no spinner.
 */
export interface PatchTicket { path: string; quiet: boolean; live(): boolean; done(answer: DiffAnswer): void; failed(said: string): void }
export interface DiffHooks {
  /** A file to fetch: opened, retried, asked anyway, or (quiet) its list row changed under a refresh. */
  want?(ticket: PatchTicket): void
  /** The pane's own read failed and the person asked again. */
  retry?(): void
  /** The terminal line that shows this file, or null where there is none (a checkout has no issue to name). */
  command?(file: DiffFile): string | null
}

/** The pane's words: its own, and the page's word for a read under way (`page.running`) beside the spinner. */
export const diffWords = (copy: Copy): DiffWords => ({ ...copy.diff, reading: copy.running })
/** Why a read failed, as the pane says it: the server giving up (504) apart from the rest, its own words when it gave some. */
export function failureWords(copy: Copy, read: { why: 'timeout' | 'cancelled' | 'failed'; said?: string }): string {
  if (read.why === 'timeout') return copy.diff.timeout
  if (read.why === 'cancelled') return copy.cancelled
  return read.said ? `${copy.failed} ${read.said}` : copy.failed
}
/**
 * The terminal line that shows one file of an issue's diff, to paste and run: the path in single quotes, a quote inside
 * it written `'\''`, joined to its flag with `=` so a path that starts with `-` stays the flag's value (clap reads
 * `--path -x` as a flag `-x`). The id and the scope are the ones the page reads with (the server checked the id; the
 * scope is one of three words), and the path a row of the server's own list.
 */
export function terminalLine(id: string, scope: string, path: string, worktree = false): string {
  return `moai diff ${id} --scope ${scope} --path='${path.replace(/'/g, `'\\''`)}'${worktree ? ' --worktree' : ''}`
}

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
/** The changed lines a head counts — what "too large" weighs. */
export const changedLines = (file: DiffFile): number => (file.adds ?? 0) + (file.dels ?? 0)
/** What a refresh compares: the head as the list gave it. A file whose head is the same keeps its patch. */
const headKey = (file: DiffFile): string => JSON.stringify([file.path, file.old_path, file.status, file.adds, file.dels, file.binary, file.mark ?? null])

type Body =
  | { kind: 'idle' }
  | { kind: 'loading' }
  | { kind: 'loaded'; file: DiffFile; cut: boolean; plain: boolean; text: string }
  | { kind: 'failed'; said: string }
interface Entry { head: DiffFile; key: string; body: Body; force: boolean; ticket: object | null; dead: boolean; details: HTMLDetailsElement | null; drawn: Body | null; version: number; timer?: ReturnType<typeof setTimeout> }
type Pane = { kind: 'blank' } | { kind: 'reading' } | { kind: 'failed'; said: string } | { kind: 'list'; truncated: boolean }

/** The body of one answer for the head it was asked for: its own file, how it was cut, whether it came without colours. */
function bodyOf(answer: DiffAnswer, head: DiffFile): Body {
  if (answer.vcs_error) return { kind: 'failed', said: answer.vcs_error.said }
  const file = answer.files.find(f => f.path === head.path) ?? answer.files[0] ?? { ...head, hunks: [] }
  // `truncated` also says the list was cut at 5,000 files; the patch itself was cut only if lines are missing.
  const counted = (file.hunks ?? []).reduce((n, h) => n + h.lines.filter(l => l.kind === 'add' || l.kind === 'delete').length, 0)
  const cut = answer.truncated && (head.adds === null || head.dels === null || counted < changedLines(head))
  const plain = answer.highlight === 'none' || (answer.highlight === 'partial' && (file.hunks ?? []).every(h => h.lines.every(l => l.spans == null)))
  return { kind: 'loaded', file, cut, plain, text: JSON.stringify(file) }
}

export function createDiff(doc: Document, words: DiffWords, onLayout?: (layout: DiffLayout) => void, states: { on: string; off: string; diff?: string } = { on: 'on', off: 'off' }, hooks: DiffHooks = {}) {
  const root = el(doc, 'section', { class: 'm-diff', tabindex: 0 })
  const toolbar = el(doc, 'div', { class: 'm-diff-toolbar m-panehead' })
  /** What the pane is showing: the file tab writes the path here, the list's pane puts its scope buttons here. */
  const slot = el(doc, 'span', { class: 'm-diff-slot' })
  const unifiedButton = iconButton(doc, { icon: 'rows-2', label: stateLabel(words.unified, true, states), pressed: true })
  const sideButton = iconButton(doc, { icon: 'columns-2', label: stateLabel(words.side, false, states), pressed: false })
  const notice = el(doc, 'span', { role: 'status' })
  const body = el(doc, 'div', { class: 'm-diff-body' })
  toolbar.append(el(doc, 'b', {}, states.diff ?? 'diff'), slot, notice, el(doc, 'span', { class: 'm-panehead-sp' }), unifiedButton, sideButton); root.append(toolbar, body)
  let layout: DiffLayout = 'unified', width = Infinity
  let pane: Pane = { kind: 'blank' }, entries: Entry[] = []
  const open = new Set<string>()
  let defaultOpen = true, restoreScroll: { top: number; left: number } | null = null
  /** The list's spinner row: it holds its height from the start, so the spinner standing later moves nothing. */
  const paneRow = el(doc, 'div', { class: 'm-diff-state' })
  let paneTimer: ReturnType<typeof setTimeout> | undefined, paneSpinning = false
  const spinner = () => [el(doc, 'span', { class: 'm-spin m-diff-spin', 'aria-hidden': 'true' }, '⠋'), el(doc, 'span', {}, words.reading)]
  const watch = () => root.dispatchEvent(new doc.defaultView!.Event('moai-file-watch', { bubbles: true }))
  const side = () => layout === 'side-by-side' && width >= 640

  /** A dead entry stops everything it had going: its read's answer, its spinner, and a chunked draw still under way. */
  function discard(entry: Entry): void { entry.dead = true; entry.ticket = null; clearTimeout(entry.timer); entry.version++; entry.details = null }
  function setPane(next: Pane): void {
    for (const entry of entries) discard(entry)
    entries = []
    if (next.kind !== 'reading') { clearTimeout(paneTimer); paneSpinning = false; paneRow.replaceChildren() }
    pane = next
  }
  /** Ask for a file's patch. A loud read shows the file as loading; a quiet one (a refresh) keeps what is drawn. */
  function request(entry: Entry, quiet: boolean): void {
    if (!hooks.want) return
    const ticket = {}
    entry.ticket = ticket
    if (!quiet) entry.body = { kind: 'loading' }
    const live = () => !entry.dead && entry.ticket === ticket
    hooks.want({
      path: entry.head.path, quiet, live,
      done(answer) {
        if (!live()) return
        entry.ticket = null
        const next = bodyOf(answer, entry.head)
        // The same patch again leaves the drawn lines alone: a refresh that found nothing new rebuilds nothing.
        if (entry.body.kind === 'loaded' && next.kind === 'loaded' && entry.body.text === next.text && entry.body.cut === next.cut && entry.body.plain === next.plain) return
        entry.body = next
        fill(entry, quiet ? { top: body.scrollTop, left: body.scrollLeft } : null)
      },
      failed(said) {
        if (!live()) return
        entry.ticket = null
        // A refresh that failed leaves the patch it would have replaced.
        if (quiet && entry.body.kind === 'loaded') return
        entry.body = { kind: 'failed', said }
        fill(entry, null)
      },
    })
  }
  const large = (entry: Entry) => !entry.force && changedLines(entry.head) > LARGE_LINES
  /** Whether opening it asks for its patch: a binary file, one too large, or one with no line changed is told from its head. */
  const wanted = (entry: Entry) => !entry.head.binary && !large(entry) && !(entry.head.adds === 0 && entry.head.dels === 0)
  function stateRow(cls: string, ...children: Array<Node | string>): HTMLElement { return el(doc, 'div', { class: `m-diff-state${cls ? ` ${cls}` : ''}` }, ...children) }
  function failRow(said: string, again: (() => void) | undefined): HTMLElement {
    const row = stateRow('m-diff-failed', el(doc, 'span', {}, said))
    if (again) { const button = el(doc, 'button', { type: 'button', class: 'm-diff-retry' }, words.retry); button.addEventListener('click', again); row.append(button) }
    return row
  }
  /** Draw a file's body as its state says, asking for the patch if it is open and not yet asked for. Closed files draw nothing. */
  function fill(entry: Entry, scroll: { top: number; left: number } | null): void {
    const details = entry.details
    if (!details || !details.open) return
    if (entry.body.kind === 'idle' && wanted(entry)) request(entry, false)
    if (entry.drawn === entry.body) return
    const shown = entry.body; entry.drawn = shown
    const version = ++entry.version
    clearTimeout(entry.timer)
    for (const child of [...details.children]) if (child.tagName !== 'SUMMARY') child.remove()
    details.removeAttribute('aria-busy')
    const head = entry.head
    if (head.binary) { details.append(el(doc, 'p', { class: 'm-diff-empty' }, words.binary)); return }
    if (large(entry)) {
      const anyway = el(doc, 'button', { type: 'button', class: 'm-diff-retry' }, words.anyway)
      anyway.addEventListener('click', () => { entry.force = true; entry.body = { kind: 'idle' }; fill(entry, null) })
      const line = hooks.command?.(head)
      details.append(stateRow('m-diff-large', el(doc, 'span', {}, fillWords(words.large, { n: changedLines(head).toLocaleString('en-US') })), anyway,
        ...(line ? [el(doc, 'span', { class: 'm-diff-cmd' }, el(doc, 'span', {}, words.terminal), el(doc, 'code', {}, line))] : [])))
      return
    }
    if (!wanted(entry) && shown.kind !== 'loaded') { details.append(el(doc, 'p', { class: 'm-diff-empty' }, words.empty)); return }
    if (shown.kind === 'idle') return
    if (shown.kind === 'loading') {
      details.setAttribute('aria-busy', 'true')
      const row = stateRow(''); details.append(row)
      entry.timer = setTimeout(() => { if (entry.drawn === shown && entry.version === version) row.replaceChildren(...spinner()) }, SPIN_DELAY_MS)
      return
    }
    if (shown.kind === 'failed') { details.append(failRow(shown.said, () => { if (entry.body === shown) request(entry, false); fill(entry, null) })); return }
    const file = shown.file
    const notes = () => {
      if (shown.cut) details.append(el(doc, 'p', { class: 'm-diff-empty' }, words.truncated))
      if (shown.plain) details.append(el(doc, 'p', { class: 'm-diff-empty' }, words.plain))
    }
    if (file.binary || !file.hunks?.length) { details.append(el(doc, 'p', { class: 'm-diff-empty' }, file.binary ? words.binary : words.empty)); notes(); return }
    const sided = side()
    const tasks: Array<() => Node[]> = []
    for (const hunk of file.hunks) {
      tasks.push(() => [el(doc, 'div', { class: 'm-diff-hunk' }, `@@ -${hunk.old_start},${hunk.old_len} +${hunk.new_start},${hunk.new_len} @@ ${hunk.section}`)])
      if (sided) {
        const grid = el(doc, 'div', { class: 'm-diff-pairs' })
        tasks.push(() => [grid])
        for (const [old, next] of pair(hunk.lines)) tasks.push(() => { grid.append(lineElement(doc, old, 'old'), lineElement(doc, next, 'new')); return [] })
      } else for (const line of hunk.lines) tasks.push(() => [lineElement(doc, line)])
    }
    // A large body is drawn 200 rows a turn, so the page answers between them; a file asked anyway takes the same road.
    let at = 0
    const chunk = () => {
      if (entry.version !== version || entry.details !== details) return
      const fragment = doc.createDocumentFragment()
      for (const end = Math.min(at + 200, tasks.length); at < end; at++) fragment.append(...tasks[at]!())
      details.append(fragment)
      if (scroll) { body.scrollTop = scroll.top; body.scrollLeft = scroll.left }
      if (at < tasks.length) setTimeout(chunk, 0); else notes()
    }
    chunk()
  }
  function paint(): void {
    // The offsets a refresh kept belong to the files open now; a file opened later starts where the person is.
    const restore = restoreScroll; restoreScroll = null
    paintState(unifiedButton, words.unified, layout === 'unified', states); paintState(sideButton, words.side, layout === 'side-by-side', states)
    notice.textContent = layout === 'side-by-side' && !side() ? words.narrow : ''
    for (const entry of entries) { clearTimeout(entry.timer); entry.details = null; entry.drawn = null }
    body.replaceChildren(); body.removeAttribute('aria-busy')
    if (pane.kind === 'blank') return
    if (pane.kind === 'reading') { body.setAttribute('aria-busy', 'true'); body.append(paneRow); return }
    if (pane.kind === 'failed') { body.append(failRow(pane.said, hooks.retry)); return }
    if (!entries.length) { body.append(el(doc, 'p', { class: 'm-diff-empty' }, words.empty)); return }
    entries.forEach((entry, index) => {
      const file = entry.head
      const details = el(doc, 'details', { class: 'm-diff-file', 'data-path': file.path })
      details.open = open.has(file.path) || (defaultOpen && index === 0 && open.size === 0)
      const name = file.old_path && file.old_path !== file.path ? `${file.old_path} → ${file.path}` : file.path
      const mark = file.mark !== undefined && MARKS.has(file.mark) ? file.mark : ''
      // Chip, path and counts are text: nothing in the head takes a click, so the summary's own toggle stays the only one.
      details.append(el(doc, 'summary', { class: 'm-diff-head' },
        el(doc, 'span', { class: `m-diff-chip${mark ? ` m-mark-${mark}` : ''}` }, mark),
        el(doc, 'span', { class: 'm-diff-path' }, name),
        el(doc, 'span', { class: 'm-diff-counts' }, el(doc, 'span', { class: 'm-count-add' }, `+${file.adds ?? 0}`), el(doc, 'span', { class: 'm-count-del' }, `−${file.dels ?? 0}`))))
      entry.details = details
      details.addEventListener('toggle', () => {
        if (entry.details !== details) return
        if (details.open) { open.add(file.path); fill(entry, null) } else open.delete(file.path)
        watch()
      })
      body.append(details)
      if (details.open) fill(entry, restore)
    })
    if (pane.truncated) body.append(el(doc, 'p', { class: 'm-diff-empty' }, words.truncated))
  }
  /**
   * The heads of a list. With `preserve` (a refresh) the open files, the scroll and every patch whose head is the same
   * stay; an open file whose head changed is fetched again quietly, keeping its old lines until the new ones come. The
   * same list again touches nothing. A refresh that lands on a pane showing no list (still reading, or failed) has
   * nothing to keep, so it opens the first file as a first read does.
   */
  function heads(list: DiffList, preserve: boolean, preloaded?: (head: DiffFile) => Body | null): void {
    const keep = preserve && pane.kind === 'list'
    const same = (entry: Entry, head: DiffFile) => {
      if (entry.key !== headKey(head)) return false
      const given = preloaded?.(head)
      return !given || (given.kind === 'loaded' && entry.body.kind === 'loaded' && given.text === entry.body.text)
    }
    if (keep && pane.kind === 'list' /* narrows `pane` */ && pane.truncated === list.truncated && entries.length === list.files.length && entries.every((entry, i) => same(entry, list.files[i]!))) return
    const top = body.scrollTop, left = body.scrollLeft
    if (keep) for (const details of body.querySelectorAll<HTMLDetailsElement>('details')) {
      const path = details.dataset.path; if (path) { if (details.open) open.add(path); else open.delete(path) }
    } else open.clear()
    const old = keep ? new Map(entries.map(entry => [entry.head.path, entry])) : new Map<string, Entry>()
    const kept = new Set<Entry>(), again: Entry[] = []
    const next = list.files.map(head => {
      const key = headKey(head), was = old.get(head.path), given = preloaded?.(head) ?? null
      if (was && same(was, head)) { kept.add(was); return was }
      const entry: Entry = { head, key, body: given ?? { kind: 'idle' }, force: was?.force ?? false, ticket: null, dead: false, details: null, drawn: null, version: 0 }
      // A changed file that was open keeps its old lines on screen until its new patch comes — only while it still
      // asks for one: a head now binary, too large or with no line changed is told from the head, not the old lines.
      if (!given && was?.body.kind === 'loaded' && open.has(head.path) && wanted(entry)) { entry.body = was.body; again.push(entry) }
      return entry
    })
    for (const entry of entries) if (!kept.has(entry)) discard(entry)
    entries = []
    setPane({ kind: 'list', truncated: list.truncated })
    entries = next
    defaultOpen = !keep; restoreScroll = keep ? { top, left } : null
    paint(); if (keep) { body.scrollTop = top; body.scrollLeft = left }
    for (const entry of again) if (entry.details?.open && wanted(entry)) request(entry, true)
  }
  const choose = (next: DiffLayout) => { if (layout === next) return; layout = next; onLayout?.(layout); paint() }
  unifiedButton.addEventListener('click', () => choose('unified'))
  sideButton.addEventListener('click', () => choose('side-by-side'))
  return {
    el: root, slot,
    /** A whole answer at once, every file with its patch (`hunks`); a file without is fetched when opened. */
    show(value: DiffAnswer, preserve = false) {
      if (value.vcs_error) { setPane({ kind: 'failed', said: value.vcs_error.said }); paint(); return }
      heads(value, preserve, head => head.hunks || head.binary ? bodyOf({ ...value, files: [head] }, head) : null)
    },
    /** The first step's answer: every head now, each patch when its file opens. */
    list(value: DiffList, preserve = false) { heads(value, preserve) },
    /** The list is being read. Says nothing until the read has run a moment, then a spinner and its word; never "no changes". */
    reading() {
      if (pane.kind === 'reading') {
        // A read that went out after the cursor settled: its own moment starts now, unless the spinner already stands.
        if (!paneSpinning) { clearTimeout(paneTimer); paneTimer = setTimeout(() => { paneSpinning = true; paneRow.replaceChildren(...spinner()) }, SPIN_DELAY_MS) }
        return
      }
      setPane({ kind: 'reading' }); open.clear(); defaultOpen = true; paint()
      clearTimeout(paneTimer); paneTimer = setTimeout(() => { paneSpinning = true; paneRow.replaceChildren(...spinner()) }, SPIN_DELAY_MS)
    },
    /** The list's read failed: why, and a button that asks again (`hooks.retry`). */
    failed(said: string) { setPane({ kind: 'failed', said }); paint() },
    /** Nothing to show (the pane is hidden or has no issue): no word, no spinner, every pending read dead. */
    clear() { setPane({ kind: 'blank' }); paint() },
    /** Fetch an open file again quietly (its text changed on disk), unless a read of it is out. */
    reload(path: string) {
      const entry = entries.find(e => e.head.path === path)
      if (!entry || entry.ticket || !entry.details?.open || !wanted(entry)) return
      if (entry.body.kind === 'loaded') request(entry, true)
      else if (entry.body.kind === 'failed') { request(entry, false); fill(entry, null) }
    },
    setHooks(next: DiffHooks) { hooks = next },
    setLayout(value: DiffLayout) { if (layout !== value) { layout = value; paint() } },
    setWidth(value: number) { const changed = (width >= 640) !== (value >= 640); width = value || Infinity; if (changed) paint() },
    focus() { root.focus() },
    visiblePath(): string | undefined { return body.querySelector<HTMLDetailsElement>('details[open]')?.dataset.path },
    command(action: string): boolean {
      if (action === 'toggle') { (layout === 'unified' ? sideButton : unifiedButton).click(); return true }
      if (action === 'left' || action === 'right') { body.scrollLeft += action === 'left' ? -80 : 80; return true }
      if (action === 'next' || action === 'prev') { const files = [...body.querySelectorAll<HTMLElement>('summary')]; const at = files.indexOf(doc.activeElement as HTMLElement); files[at < 0 ? (action === 'next' ? 0 : files.length - 1) : (at + (action === 'next' ? 1 : files.length - 1)) % files.length]?.focus(); return files.length > 0 }
      return false
    },
  }
}
