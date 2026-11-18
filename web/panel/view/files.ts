import { filesSource, changeFingerprint, type Entry, type FileText, type SearchResult, type Checkout, type Change, type ChangeSet, type Mark } from '../data/files.ts'
import { fileGroup, fileGroupTable, type FileGroupTable } from '../data/file-groups.ts'
import { el, icon, iconButton, paintState, stateLabel } from './components.ts'
import { createSplit } from './split.ts'
import { createDiff, diffWords, failureWords, type PatchTicket } from './diff.ts'
import { checkoutPatch, patchQueue, PATCH_GROUP } from '../data/changes.ts'
import { twigParts, TWIG_TEXT } from './list-model.ts'
import { mountDetail } from './detail.ts'
import { loadDetail } from '../data/source.ts'
import type { FileWatch, StampAnswer } from '../data/files-refresh.ts'
import type { State } from '../store.ts'
import type { IssueIndex } from '../data/index.ts'
import type { PaneView, ViewContext } from './types.ts'

const CLASSES = new Set(['comment', 'string', 'number', 'keyword', 'type', 'function', 'constant', 'tag', 'attribute', 'heading', 'other'])
/** How long a file's patch waits for the cursor to settle before it is asked for. */
const PATCH_PAUSE_MS = 100
const join = (parent: string, name: string) => parent === '.' ? name : `${parent}/${name}`
const KIND_CLASSES = new Set(['directory', 'link', 'source', 'doc', 'data', 'script', 'binary', 'plain'])
/** Closed list: a received group name never becomes a class by itself. */
const fileClass = (entry: Entry, table: FileGroupTable | null): string => {
  if (entry.kind === 'dir') return 'directory'
  if (entry.kind === 'link') return 'link'
  const group: string = fileGroup(entry.name, table)
  return KIND_CLASSES.has(group) ? group : 'plain'
}

export function mountFiles(root: HTMLElement, ctx: ViewContext): PaneView {
  const doc = ctx.document
  const words = ctx.copy.files
  const source = ctx.client ? filesSource(ctx.client) : null
  const split = createSplit(doc, { position: 'right', ratios: { right: 0.5 }, onRatio: (position, ratio) => { if (position === 'right') ctx.intents.dispatch({ type: 'files-ratio', ratio }) } })
  const tree = el(doc, 'div', { class: 'm-files-tree', role: 'tree', tabindex: 0 })
  const content = el(doc, 'div', { class: 'm-files-content', tabindex: 0 })
  const bar = el(doc, 'div', { class: 'm-files-toolbar' })
  const checkoutSelect = el(doc, 'select', { 'aria-label': words.checkout })
  /** Git or svn, where both stand over the repository (moai-0d0s.qg0): drawn only when `/api/checkouts` names both. */
  const vcsSelect = el(doc, 'select', { 'aria-label': words.vcs })
  const changedButton = iconButton(doc, { icon: 'file-diff', label: stateLabel(words.changed, false, words), pressed: false })
  const diffButton = iconButton(doc, { icon: 'diff', label: stateLabel(words.diff, false, words), pressed: false })
  const detailButton = iconButton(doc, { icon: 'panel-right', label: stateLabel(words.detail, false, words), pressed: false })
  const search = el(doc, 'input', { class: 'm-files-search', type: 'search', placeholder: words.search, 'aria-label': words.search })
  const sep = el(doc, 'span', { class: 'm-files-sep', 'aria-hidden': 'true' })
  bar.append(el(doc, 'span', { class: 'm-files-field m-files-checkout' }, icon(doc, 'git-branch'), checkoutSelect), el(doc, 'span', { class: 'm-files-field m-files-find' }, icon(doc, 'search'), search), sep, changedButton, diffButton, detailButton)
  const right = el(doc, 'div', { class: 'm-files-right' })
  /** The content pane's head: the chosen file's path (nothing else: the endpoint tells no language or line count). */
  const contentPath = el(doc, 'b', {})
  const contentHead = el(doc, 'div', { class: 'm-panehead m-files-head' }, contentPath)
  const collapsedNotice = el(doc, 'p', { class: 'm-files-collapsed', role: 'status' }, words.collapsed)
  const inner = createSplit(doc, { position: 'top' })
  const issueRoot = el(doc, 'div', { class: 'm-files-issue', tabindex: 0 })
  const issueHost = el(doc, 'div', {})
  const issueEmpty = el(doc, 'p', {}, words.noIssue)
  issueRoot.append(issueHost, issueEmpty)
  const issueView = mountDetail(issueHost, { ...ctx, intents: { ...ctx.intents, closeDetail: () => { detailOn = false; paintRight() } } })
  // A checkout has no issue to name, so no terminal line: the too-large row offers the fetch alone.
  // Retry after the checkout's change list said why it failed reads that list again: the one held is the failure.
  const diff = createDiff(doc, diffWords(ctx.copy), layout => ctx.intents.dispatch({ type: 'diff-layout', layout }), words, { want: wantPatch, retry: () => { if (changeError) { held = null; changesLoaded = false } void showDiff().catch(() => {}) } })
  /** The pane's patch reads: two at a time, all dropped when the cursor or the checkout moves. */
  const patches = patchQueue(2)
  function dropPatches(): void { patches.clear(); ctx.client?.cancelGroup?.(PATCH_GROUP.files) }
  function wantPatch(ticket: PatchTicket): void {
    const client = ctx.client, key = rootKey
    if (!client || key === undefined) { ticket.failed(ctx.copy.failed); return }
    const job = { live: () => ticket.live() && key === rootKey, async run() {
      const read = await checkoutPatch(client, key, ticket.path, ticket.quiet, vcsOf(key))
      if (key !== rootKey) return
      if (read.ok) ticket.done(read.value); else ticket.failed(failureWords(ctx.copy, read))
    } }
    // A patch asked as the cursor passes waits a moment first: one the cursor moved past by then is never sent.
    if (ticket.quiet) patches.add(job); else setTimeout(() => { if (job.live()) patches.add(job) }, PATCH_PAUSE_MS)
  }
  inner.detail.append(issueRoot); inner.list.append(diff.el)
  right.append(contentHead, content); split.detail.append(right)

  const message = el(doc, 'div', { class: 'm-files-message', role: 'status' })
  split.list.append(message, tree); root.append(bar, split.el)
  const directories = new Map<string, Entry[]>()
  const expanded = new Set<string>(['.'])
  const partial = new Set<string>()
  const stamps = new Map<string, string | null>()
  const readTimes = new Map<string, number>()
  let fileStamp: string | null = null, fileReadAt = 0
  let lastFile: FileText | null = null
  let beforeSearch = new Set<string>()
  let matches: SearchResult | null = null
  let cursor = 0, chosen = '', base = '.', generation = 0, pendingG = false, timer: ReturnType<typeof setTimeout> | undefined
  let rows: Array<{ path: string; entry: Entry; depth: number; twig: boolean[] }> = []
  /** Changed-only: each directory's total of the `+N −M` below it, summed once when the tree is built. */
  let sums = new Map<string, { adds: number; dels: number }>()
  let alive = true, started = false, rootKey: string | undefined, checkouts: Checkout[] = [], changedOnly = false, diffOn = false, detailOn = false
  /** The tab's git·svn pick — this tab's alone, never kept; undefined while the repository has one backend. */
  let vcs: string | undefined
  /**
   * The checkout the pick belongs to: the one `/api/checkouts` calls current, the only one its `vcs` key speaks of. Another
   * checkout may stand outside the svn working copy, and a pick it does not have is refused (400) on every read — so
   * there the pick is not sent, the server chooses as it would with no control, and the control steps aside.
   */
  let vcsHome: string | undefined
  const vcsField = el(doc, 'span', { class: 'm-files-field m-files-vcs' }, vcsSelect)
  /** The pick to send with a read of `key`'s checkout, or undefined where the choice is not offered. */
  const vcsOf = (key: string | undefined): string | undefined => key !== undefined && key === vcsHome ? vcs : undefined
  let groups: FileGroupTable | null = null, groupsBoot: unknown
  /** The checkout's change list as the server last gave it, read whenever this tab stands; the letters and the diff heads come from it. */
  let changes: Change[] = [], dirMarks = new Map<string, Mark>(), changeFp = '', changeError = '', changeNote = '', held: ChangeSet | null = null, changesPending: { key: string; read: Promise<ChangeSet | null>; background: boolean } | null = null, ordinary: Map<string, Entry[]> | null = null, stateNow: State | null = null, indexNow: IssueIndex | null = null
  let issueShown: State['shown'] = { value: null, loading: false, problem: null }
  let diffEpoch = 0, issueEpoch = 0, changesLoaded = false, painting = 0
  /** Changed-only raised the diff pane itself; any hand on the pane clears this, and leaving turns the pane off only while it stands. */
  let autoDiff = false
  function setDiff(on: boolean): void { diffOn = on; paintRight(); if (on) void showDiff().catch(() => {}); else { diffEpoch++; dropPatches(); diff.clear() } watchChanged() }
  /** Leaving changed-only: undo what entering did, and only that. */
  function releaseDiff(): void { if (autoDiff) { autoDiff = false; if (diffOn) setDiff(false) } }
  let ordinaryExpanded = new Set<string>(), ordinaryBase = '.'
  /** Put `nodes` in `parent` only when they are not already its children: moving a pane drops its scroll and focus. */
  const place = (parent: HTMLElement, ...nodes: HTMLElement[]) => {
    if (parent.children.length !== nodes.length || nodes.some((node, i) => parent.children[i] !== node)) parent.replaceChildren(...nodes)
  }
  function paintRight(): void {
    paintState(diffButton, words.diff, diffOn, words); paintState(detailButton, words.detail, detailOn, words); paintState(changedButton, words.changed, changedOnly, words)
    const narrow = root.clientWidth > 0 && root.clientWidth < 640
    if (detailOn && diffOn && !narrow) { place(inner.detail, issueRoot); place(inner.list, diff.el); place(right, inner.el) }
    else if (detailOn) { if (diffOn && narrow) place(right, collapsedNotice, issueRoot); else place(right, issueRoot) }
    else if (diffOn) place(right, diff.el)
    else place(right, contentHead, content)
    const hasIssue = !!checkouts.find(c => c.key === rootKey)?.issue
    issueHost.hidden = !hasIssue; issueEmpty.hidden = hasIssue
    if (stateNow && detailOn && hasIssue) issueView.render({ ...stateNow, shown: issueShown }, indexNow)
  }
  async function readIssue(): Promise<void> {
    const key = rootKey, issue = checkouts.find(c => c.key === key)?.issue, epoch = ++issueEpoch
    if (!issue || !ctx.client) { issueShown = { value: null, loading: false, problem: null }; paintRight(); return }
    const loaded = await loadDetail(ctx.client, issue)
    if (epoch !== issueEpoch || key !== rootKey) return
    issueShown = loaded.ok ? { value: { id: issue, row: loaded.value }, loading: false, problem: null } : { value: null, loading: false, problem: loaded.outcome }
    paintRight()
  }
  const watchChanged = () => root.dispatchEvent(new doc.defaultView!.Event('moai-file-watch', { bubbles: true }))
  async function switchCheckout(key: string): Promise<void> {
    if (!checkouts.some(c => c.key === key)) return
    generation++; diffEpoch++; issueEpoch++; clearTimeout(timer); rootKey = key; vcsField.hidden = vcsOf(key) === undefined; chosen = ''; cursor = 0; base = '.'; matches = null; clearChanges(); ordinary = null; changedOnly = false; releaseDiff()
    directories.clear(); partial.clear(); stamps.clear(); readTimes.clear(); fileStamp = null; fileReadAt = 0; lastFile = null; dropPatches();  expanded.clear(); expanded.add('.'); content.replaceChildren(); contentPath.textContent = ''; diff.clear(); draw(); paintRight()
    void ensureChanges(key).then(set => { if (set && key === rootKey && !changedOnly) draw(rows[cursor]?.path) })
    try { await load('.'); if (rootKey === key) { draw(); if (diffOn) void showDiff().catch(() => {}) } } catch { message.textContent = words.directoryError; const current = checkouts.find(c => c.current); if (current && current.key !== key) { checkoutSelect.value = current.key; void switchCheckout(current.key) } }
    void readIssue(); watchChanged()
  }
  /** What the tree and the diff pane stand on for a checkout whose list is not read yet: nothing, and the note it wrote goes. */
  function clearChanges(): void {
    changes = []; dirMarks = new Map(); held = null; changeFp = ''; changeError = ''; changesLoaded = false; changesPending = null
    if (changeNote && message.textContent === changeNote) message.textContent = ''
    changeNote = ''
  }
  /** Takes an answer in. The note (a read that failed, or stopped at the cap) is written only when the answer changes — never per step. */
  function applyChanges(set: ChangeSet, fingerprint = changeFingerprint(set)): void {
    held = set; changes = set.files; dirMarks = new Map(set.dirs); changeError = set.error ?? ''; changeFp = fingerprint; changesLoaded = true
    const note = set.error ? set.error : set.truncated ? `${set.total} · ${words.truncated}` : ''
    if (note) { message.textContent = note; changeNote = note }
    else { if (changeNote && message.textContent === changeNote) message.textContent = ''; changeNote = '' }
  }
  /**
   * The checkout's list, read once however many ask at once (open, toggle, diff pane); null for a read that failed or
   * went stale. A read the person waits on never rides a refresh's: the refresher cancels its own group whenever its
   * watch moves, and the toggle would then say the read failed.
   */
  function ensureChanges(key: string, background = false): Promise<ChangeSet | null> {
    if (!source || key !== rootKey) return Promise.resolve(null)
    if (held) return Promise.resolve(held)
    if (changesPending?.key === key && (!changesPending.background || background)) return changesPending.read
    const read: Promise<ChangeSet | null> = source.changes(key, background, vcsOf(key)).then(set => { if (key !== rootKey) return null; applyChanges(set); return set }, () => null)
      .finally(() => { if (changesPending?.read === read) changesPending = null })
    changesPending = { key, read, background }
    return read
  }
  async function toggleChanged(): Promise<void> {
    if (!source || rootKey === undefined) return
    if (changedOnly && ordinary) { directories.clear(); for (const [k, v] of ordinary) directories.set(k, v); ordinary = null; changedOnly = false; base = ordinaryBase; expanded.clear(); for (const name of ordinaryExpanded) expanded.add(name); cursor = 0; draw(); paintRight(); releaseDiff(); if (diffOn) void showDiff().catch(() => {}); return }
    const key = rootKey; const result = await ensureChanges(key)
    // A second press while the first waited on the list: the first has entered already, and entering again would keep
    // the changed tree as the one to come back to.
    if (key !== rootKey || changedOnly) return
    if (!result) { message.textContent = words.readError; return }
    ordinary = new Map(directories); ordinaryExpanded = new Set(expanded); ordinaryBase = base; base = '.'
    buildChanged(changes); changedOnly = true; cursor = 0; draw(); paintRight(); watchChanged()
    if (!diffOn) { autoDiff = true; setDiff(true) } else void showDiff().catch(() => {})
  }
  function buildChanged(next: Change[]): void {
    changes = next; directories.clear(); directories.set('.', []); expanded.clear(); expanded.add('.'); sums = new Map()
    for (const change of changes) {
      const parts = change.path.split('/'); if (parts.some(p => !p || p === '.' || p === '..')) continue
      let dir = '.'
      parts.forEach((name, i) => {
        if (i < parts.length - 1) { const at = join(dir, name), sum = sums.get(at) ?? { adds: 0, dels: 0 }; sum.adds += change.adds ?? 0; sum.dels += change.dels ?? 0; sums.set(at, sum) }
        const last = i === parts.length - 1, entries = directories.get(dir) ?? []
        if (!entries.some(e => e.name === name)) entries.push({ name, kind: last ? 'file' : 'dir', openable: true, size: 0 })
        directories.set(dir, entries)
        if (!last) { dir = join(dir, name); if (!directories.has(dir)) directories.set(dir, []); expanded.add(dir) }
      })
    }
    for (const entries of directories.values()) entries.sort((a, b) => Number(b.kind === 'dir') - Number(a.kind === 'dir') || a.name.localeCompare(b.name))
  }
  /**
   * The cursor's row in the diff pane, in two steps as the list pane's: the heads come from the checkout's change list
   * (read once, then kept by the refresh), each patch when its file opens. A file row heads itself; a directory row
   * heads the changed files under it; anything else changed nothing. Rejects only for a refresh that failed.
   */
  async function showDiff(background = false): Promise<void> {
    const row = rows[cursor], key = rootKey
    if (row) diff.slot.textContent = row.path
    if (!source || !row || key === undefined) return
    const epoch = ++diffEpoch
    if (!background) dropPatches()
    if (!changesLoaded) {
      if (!background) diff.reading()
      const found = await ensureChanges(key, background)
      if (epoch !== diffEpoch || key !== rootKey) return
      if (!found) { if (background) throw new Error('changes'); diff.failed(ctx.copy.failed); return }
    }
    if (changeError) { diff.failed(changeError); return }
    const folder = row.entry.kind === 'dir' || (row.entry.kind === 'link' && row.entry.dir === true && row.entry.openable)
    const under = folder ? `${row.path}/` : null
    diff.list({ files: changes.filter(change => under ? change.path.startsWith(under) : change.path === row.path), truncated: false }, background)
  }
  checkoutSelect.addEventListener('change', () => void switchCheckout(checkoutSelect.value))
  // Another backend is another reading of the same checkout: everything read goes, as a checkout switch does.
  vcsSelect.addEventListener('change', () => { vcs = vcsSelect.value; if (rootKey !== undefined) void switchCheckout(rootKey) })
  // `SPC w` puts the focus here, and the select's type-ahead would let the next letter switch the checkout: an old
  // hand's `SPC w w` (the pane move before `SPC x`), and every worktree branch starts `worktree-`. So no letter
  // reaches it; choosing is the arrows and Enter, or a click.
  checkoutSelect.addEventListener('keydown', event => {
    if (event.ctrlKey || event.metaKey || event.altKey) return
    if (event.isComposing || event.key === 'Process' || (event.key.length === 1 && event.key !== ' ')) event.preventDefault()
  })
  changedButton.addEventListener('click', () => void toggleChanged())
  diffButton.addEventListener('click', () => { autoDiff = false; setDiff(!diffOn) })
  detailButton.addEventListener('click', () => { detailOn = !detailOn; paintRight(); if (detailOn) void readIssue(); watchChanged() })
  async function load(dir: string, limit?: number): Promise<number> {
    if (!source) return 0
    if (directories.has(dir) && (limit !== undefined || !partial.has(dir))) return 0
    const key = rootKey
    const result = await source.list(dir, limit, key)
    if (key !== rootKey) return 0
    directories.set(dir, result.entries); stamps.set(dir, result.stamp); readTimes.set(dir, Date.now())
    if (result.truncated) partial.add(dir); else partial.delete(dir)
    return result.entries.length
  }
  /** `kin` is the row's parent's twig; each row adds whether a sibling follows it, which is what list-model's twigParts reads. */
  function flatten(dir: string, depth: number, kin: boolean[]): void {
    const entries = directories.get(dir) ?? []
    entries.forEach((entry, i) => {
      const path = join(dir, entry.name), twig = [...kin, i < entries.length - 1]; rows.push({ path, entry, depth, twig })
      if (expanded.has(path)) flatten(path, depth + 1, twig)
    })
  }
  /** One row: [mark cell][name cell: twig, fold, name][meta]. Everything others wrote goes in as text nodes. */
  function rowNode(level: number, kind: string, selected: boolean, fold: HTMLElement, label: HTMLElement, twig: boolean[], mark: string, markClass: string, meta: Array<Node | string>, expandedState?: boolean): HTMLButtonElement {
    const node = el(doc, 'button', { type: 'button', role: 'treeitem', class: 'm-row m-files-row', 'aria-level': level, 'aria-selected': String(selected), ...(expandedState === undefined ? {} : { 'aria-expanded': String(expandedState) }) })
    const twigNode = el(doc, 'span', { class: 'moai-twig', 'aria-hidden': 'true' })
    for (const part of twigParts(twig)) twigNode.append(el(doc, 'span', { class: 'moai-tw', 'data-tw': part }, TWIG_TEXT[part]))
    const name = el(doc, 'span', { class: `m-row-title m-files-name m-files-kind-${kind}` })
    if (twigNode.childNodes.length) name.append(twigNode)
    name.append(fold, label)
    node.append(el(doc, 'span', { class: `m-files-c-mark${markClass ? ` ${markClass}` : ''}` }, mark), name)
    const metaNode = el(doc, 'span', { class: 'm-row-meta m-files-meta' }); metaNode.append(...meta); node.append(metaNode)
    return node
  }
  function draw(keepPath?: string): void {
    rows = []; flatten(base, 0, [])
    if (keepPath) { const found = rows.findIndex(row => row.path === keepPath); if (found >= 0) cursor = found }
    cursor = Math.max(0, Math.min(cursor, rows.length - 1))
    const changesByPath = new Map(changes.map(change => [change.path, change]))
    const matchesByPath = new Map((matches?.matches ?? []).map(match => [match.path, match]))
    const nodes = rows.map((row, index) => {
      const isDir = row.entry.kind === 'dir' || (row.entry.kind === 'link' && row.entry.openable && row.entry.dir === true)
      const change = changedOnly ? changesByPath.get(row.path) : undefined
      // The letter is the server's: a file's from the list, a directory's from the map. A removed path is a row only in changed-only.
      const mark: Mark = isDir ? dirMarks.get(row.path) ?? '' : changesByPath.get(row.path)?.mark ?? ''
      // The fold is a span, not a button: the row is the button, and a button inside it would press twice.
      const fold = el(doc, 'span', { class: `m-files-fold${isDir ? ' moai-toggle' : ''}`, 'aria-hidden': 'true' })
      if (isDir) fold.append(icon(doc, 'chevron-right', 12))
      const label = el(doc, 'span', { class: `m-files-label${change?.mark === 'D' ? ' m-files-gone' : ''}` })
      const hit = matchesByPath.get(row.path)
      const offset = row.path.length - row.entry.name.length
      if (hit && hit.start >= offset && hit.end <= row.path.length && hit.end >= hit.start) {
        label.append(doc.createTextNode(row.entry.name.slice(0, hit.start - offset)), el(doc, 'mark', {}, row.entry.name.slice(hit.start - offset, hit.end - offset)), doc.createTextNode(row.entry.name.slice(hit.end - offset)))
      } else label.append(doc.createTextNode(row.entry.name))
      if (row.entry.kind === 'link') label.prepend(doc.createTextNode('↗ '))
      if (change && typeof change.old_path === 'string' && change.old_path) label.append(el(doc, 'span', { class: 'm-files-from' }, ` ← ${change.old_path}`))
      const meta: Array<Node | string> = []
      const total = changedOnly ? (change ?? (isDir ? sums.get(row.path) : undefined)) : undefined
      if (total) {
        const adds = total.adds ?? 0, dels = total.dels ?? 0
        const counts = el(doc, 'span', { class: isDir ? 'm-files-faint' : '' }, '')
        counts.append(el(doc, 'span', { class: 'm-count-add' }, `+${adds}`), doc.createTextNode(' '), el(doc, 'span', { class: 'm-count-del' }, `−${dels}`))
        meta.push(counts)
      }
      if (!row.entry.openable) meta.push(words.unavailable)
      const letter = !changedOnly && mark === 'D' ? '' : mark
      const button = rowNode(row.depth + 1, fileClass(row.entry, groups), index === cursor, fold, label, row.twig, letter, letter ? `m-mark-${letter}` : '', meta, isDir ? expanded.has(row.path) : undefined)
      button.disabled = !row.entry.openable
      button.addEventListener('click', () => { cursor = index; if (isDir) { generation++; clearTimeout(timer); if (diffOn) void showDiff().catch(() => {}) } if (isDir) { if (expanded.has(row.path)) { expanded.delete(row.path); draw() } else void expand() } else { select(); draw() } })
      button.addEventListener('dblclick', () => { void expand() })
      return button
    })
    if (base !== '.') {
      const fold = el(doc, 'span', { class: 'm-files-fold', 'aria-hidden': 'true' })
      const up = rowNode(1, 'plain', false, fold, el(doc, 'span', { class: 'm-files-label' }, '↰ ..'), [], '', '', [])
      up.addEventListener('click', () => { base = base.includes('/') ? base.slice(0, base.lastIndexOf('/')) : '.'; cursor = 0; draw() })
      nodes.unshift(up)
    }
    tree.replaceChildren(...nodes)
  }
  function paintFile(file: FileText, preserve = false): void {
    const top = content.scrollTop, left = content.scrollLeft, token = ++painting
    fileStamp = file.stamp ?? null; fileReadAt = Date.now(); lastFile = file
    content.replaceChildren(); contentPath.textContent = file.path
    if (file.binary) { content.textContent = words.binary; return }
    const lines = (file.text ?? '').split('\n'); let index = 0
    const drawChunk = () => {
      // The same path painted again (a refresh, picked twice) starts its own chunks; this run must stop.
      if (!alive || chosen !== file.path || token !== painting) return
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
      else { if (file.truncated) content.append(el(doc, 'p', {}, words.truncated)); if (preserve) { content.scrollTop = top; content.scrollLeft = left } }
    }; drawChunk()
  }
  /** A key moved the cursor: read what it is on and keep its row in sight (nothing focuses the row itself). */
  function moved(): void {
    select(); draw()
    tree.querySelector<HTMLElement>('[aria-selected="true"]')?.scrollIntoView?.({ block: 'nearest' })
  }
  function select(): void {
    const row = rows[cursor]; const current = ++generation; diffEpoch++; clearTimeout(timer)
    if (!row || !source) return
    // The diff pane follows every row: a directory heads the changed files under it.
    const isFile = row.entry.kind === 'file' || (row.entry.kind === 'link' && !row.entry.dir && row.entry.openable)
    // The heads come from the change list the page holds; the patches wait a moment (wantPatch), so a held `j` asks for none.
    if (diffOn) void showDiff().catch(() => {})
    if (!isFile) return
    chosen = row.path
    timer = setTimeout(() => { void source.file(row.path, rootKey).then(file => { if (alive && generation === current) paintFile(file) }).catch(() => { if (generation === current) { painting++; contentPath.textContent = row.path; content.textContent = words.readError } }) }, 100)
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
    const ignored = await source.ignored(rootKey, vcsOf(rootKey)).catch(() => [])
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
      const result = await source.search(q, base, rootKey, vcsOf(rootKey)); if (current !== generation) return
      matches = result
      for (const hit of result.matches) {
        const parts = hit.path.split('/'); parts.pop(); let dir = '.'
        for (const part of parts) { dir = join(dir, part); await load(dir); if (current !== generation || !alive) return; expanded.add(dir) }
      }
      message.textContent = `${result.total} ${words.matches}${result.truncated || result.walk_truncated ? ` (${words.truncated})` : ''}${result.ignored_unavailable ? ` (${words.ignoredUnavailable})` : ''}`; draw()
    } catch { message.textContent = words.searchError }
  }
  // The box stands in the toolbar, so a click reaches it as well as `/`: either way the folds to come back to are the
  // ones before this search — not a set an older `/` left behind.
  search.addEventListener('focus', () => { if (!matches) beforeSearch = new Set(expanded) })
  search.addEventListener('input', () => {
    clearTimeout(timer); generation++
    if (!search.value) { generation++; matches = null; expanded.clear(); for (const p of beforeSearch) expanded.add(p); message.textContent = ''; draw(); return }
    timer = setTimeout(() => void find(search.value), 200)
  })
  return {
    render(state, index) {
      stateNow = state; indexNow = index
      // The table changes only with the boot: read it once per boot, not on every render.
      if (state.boot !== groupsBoot) { groupsBoot = state.boot; groups = state.boot ? fileGroupTable((state.boot as { file_groups?: unknown }).file_groups) : null }
      split.setWidth(root.clientWidth); split.setRatio('right', state.filesRatio)
      inner.setPosition(state.detail.position === 'left' || state.detail.position === 'right' ? 'left' : 'top'); inner.setWidth(root.clientWidth)
      diff.setLayout(state.diffLayout); diff.setWidth(root.clientWidth); paintRight()
      if (!started) { started = true; void source?.checkouts().then(({ list, vcs: choice }) => {
        checkouts = [...list].sort((a, b) => Number(b.main) - Number(a.main))
        if (choice) { vcs = choice.fallback; vcsHome = checkouts.find(c => c.current)?.key; vcsSelect.replaceChildren(...choice.found.map(word => el(doc, 'option', { value: word }, word))); vcsSelect.value = vcs; bar.insertBefore(vcsField, bar.children[1] ?? null) }
        checkoutSelect.replaceChildren(...checkouts.map(c => el(doc, 'option', { value: c.key }, `${c.branch || c.key}${c.branch ? ` (${c.key})` : ''}${c.issue ? ` · ${c.issue}` : ''}`)));
        if (ctx.client) for (const checkout of checkouts) if (checkout.issue) void loadDetail(ctx.client, checkout.issue).then(result => {
          const option = [...checkoutSelect.options].find(o => o.value === checkout.key)
          if (option && result.ok && typeof result.value.title === 'string') option.textContent = `${checkout.branch || checkout.key}${checkout.branch ? ` (${checkout.key})` : ''} · ${result.value.title}`
        })
        const first = checkouts.find(c => c.current) ?? checkouts[0]; if (first) { checkoutSelect.value = first.key; void switchCheckout(first.key) } }).catch(() => { message.textContent = words.directoryError }) }
    }, focus() { tree.focus() },
    handleKey(event) {
      // These keys come before the router's own guards: an IME at work, a browser shortcut (Ctrl-L, Alt-Left) and a
      // control that takes keys itself (the checkout <select>) are not the tree's. Ctrl-d / Ctrl-u are.
      if (event.isComposing || event.metaKey || event.altKey || (event.ctrlKey && event.key !== 'd' && event.key !== 'u')) return false
      const target = event.target as HTMLElement | null
      if (target !== search && target && (target.isContentEditable || ['INPUT', 'SELECT', 'TEXTAREA'].includes(target.tagName))) return false
      if (event.target === search) {
        if (event.key === 'Escape') { search.value = ''; expanded.clear(); for (const p of beforeSearch) expanded.add(p); matches = null; message.textContent = ''; draw(); tree.focus(); return true }
        if (event.key === 'Enter') { tree.focus(); return true }
        return false
      }
      const row = rows[cursor]
      if (event.target instanceof doc.defaultView!.Node && issueRoot.contains(event.target)) return false
      if (event.target instanceof doc.defaultView!.Node && diff.el.contains(event.target)) {
        if (event.key === 'Tab') return diff.command(event.shiftKey ? 'prev' : 'next')
        if (event.key === 'H' || event.key === 'L') return diff.command(event.key === 'H' ? 'left' : 'right')
      }
      if (event.key === 'Tab') { void expandAll(); return true }
      if (event.key === 'g') { if (pendingG) { cursor = 0; pendingG = false; moved() } else pendingG = true; return true }
      pendingG = false
      if (event.key === 'H' || event.key === 'L') { content.scrollLeft += event.key === 'H' ? -80 : 80; return true }
      if (event.ctrlKey && (event.key === 'd' || event.key === 'u')) { cursor = Math.max(0, Math.min(rows.length - 1, cursor + (event.key === 'd' ? 10 : -10))); moved(); return true }
      if (event.key === '/') { if (!matches) beforeSearch = new Set(expanded); search.focus(); return true }
      if (event.key === 'Escape' && matches) { search.value = ''; expanded.clear(); for (const p of beforeSearch) expanded.add(p); matches = null; message.textContent = ''; generation++; draw(); return true }
      if (event.key === 'j' || event.key === 'ArrowDown') cursor = Math.min(rows.length - 1, cursor + 1)
      else if (event.key === 'k' || event.key === 'ArrowUp') cursor = Math.max(0, cursor - 1)
      else if (event.key === 'G') cursor = rows.length - 1
      else if (event.key === 'l' || event.key === 'ArrowRight') { void expand(); return true }
      else if (event.key === 'h' || event.key === 'ArrowLeft') { if (row && expanded.has(row.path)) expanded.delete(row.path); else if (row?.path.includes('/')) { const parent = row.path.slice(0, row.path.lastIndexOf('/')); const i = rows.findIndex(r => r.path === parent); if (i >= 0) cursor = i } }
      else if (event.key === 'Enter') { if (row && (row.entry.kind === 'dir' || (row.entry.kind === 'link' && row.entry.dir && row.entry.openable))) { base = row.path; expanded.add(base); void load(base).then(() => draw()) } else { root.classList.add('m-files-show-content'); content.focus() } return true }
      else if (event.key === 'Backspace') { if (root.classList.contains('m-files-show-content')) { root.classList.remove('m-files-show-content'); tree.focus(); return true } if (base !== '.') { base = base.includes('/') ? base.slice(0, base.lastIndexOf('/')) : '.'; cursor = 0 } }
      else if ((event.key === 'n' || event.key === 'N') && matches?.matches.length) { const found = rows.map((r, i) => matches?.matches.some(m => m.path === r.path) ? i : -1).filter(i => i >= 0); if (found.length) cursor = event.key === 'n' ? found.find(i => i > cursor) ?? found[0]! : [...found].reverse().find(i => i < cursor) ?? found[found.length - 1]! }
      else return false
      moved(); return true
    },
    fileWatch() {
      if (rootKey === undefined) return null
      const diffShown = diffOn && !(detailOn && root.clientWidth > 0 && root.clientWidth < 640)
      const contentShown = !detailOn && !diffOn
      return { key: `files:${rootKey}`, root: rootKey, dirs: changedOnly ? [] : [base, ...rows.filter(row => expanded.has(row.path)).map(row => row.path)].filter(dir => directories.has(dir)).map(path => ({ path, stamp: stamps.get(path) ?? null, readAt: readTimes.get(path) })),
        ...((chosen && (contentShown || diffShown)) ? { file: { path: chosen, stamp: fileStamp, readAt: fileReadAt } } : {}),
        // Always: the letters stand whether or not changed-only or the diff pane is on. '' until the first read lands, so that answer reads as a change.
        changeKey: vcsOf(rootKey) === undefined ? `root:${rootKey}` : JSON.stringify(['root', rootKey, vcsOf(rootKey), false]), changeFingerprint: changesLoaded ? changeFp : '' }
    },
    async refreshFileStamps(watch: FileWatch, dirs: string[], file: boolean, answer?: StampAnswer) {
      const key = rootKey; if (!source || watch.key !== `files:${key}`) return
      const selected = rows[cursor]?.path
      let treeChanged = false
      for (const dir of dirs) {
        // No stamp: the directory is gone or no longer readable. Reading it would fail on every round and hold the
        // refresh in its back-off; its parent's listing drops it, and a stamp that comes back reads it again.
        if (answer && answer.dirs[dir] === null) { stamps.set(dir, null); continue }
        const limit = partial.has(dir) ? directories.get(dir)?.length : undefined
        const listed = await source.list(dir, limit || undefined, key, true)
        if (key !== rootKey || !expanded.has(dir)) continue
        if (JSON.stringify(directories.get(dir)) !== JSON.stringify(listed.entries)) { directories.set(dir, listed.entries); treeChanged = true }
        if (listed.truncated) partial.add(dir); else partial.delete(dir)
        stamps.set(dir, listed.stamp); readTimes.set(dir, Date.now())
      }
      if (key !== rootKey) return
      if (treeChanged && !changedOnly) draw(selected)
      if (file && chosen && watch.file?.path === chosen) {
        const path = chosen
        // The shown file is gone or no longer readable: say so once instead of failing every round.
        if (answer && answer.file === null) { painting++; fileStamp = null; lastFile = null; content.textContent = words.readError; return }
        const fresh = await source.file(path, key, true); if (key === rootKey && path === chosen) { const changed = !lastFile || lastFile.path !== fresh.path || lastFile.text !== fresh.text || lastFile.binary !== fresh.binary || lastFile.truncated !== fresh.truncated || JSON.stringify(lastFile.spans) !== JSON.stringify(fresh.spans); if (changed) { paintFile(fresh, true); if (diffOn) { await showDiff(true); diff.reload(path) } } else { fileStamp = fresh.stamp ?? null; fileReadAt = Date.now(); lastFile = fresh } }
      }
    },
    async refreshFileChanges(watch: FileWatch, fingerprint: string) {
      if (watch.key !== `files:${rootKey}`) return
      const next = JSON.parse(fingerprint) as ChangeSet; const selected = rows[cursor]?.path
      applyChanges(next, fingerprint)
      if (changedOnly) buildChanged(next.files)
      draw(selected)
      if (diffOn) await showDiff(true)
    },
    filesState() { return { diff: diffOn, detail: detailOn, changed: changedOnly } }, command(action) {
      if (action === 'checkout') { checkoutSelect.focus(); checkoutSelect.click(); return true }
      if (action === 'changed') { void toggleChanged(); return true }
      if (action === 'diff') { diffButton.click(); return true }
      if (action === 'detail') { detailButton.click(); return true }
      if (action === 'diff-layout') { return diff.command('toggle') }
      if (!action.startsWith('focus.')) return false
      const side = action.slice(6)
      const show = side === 'right' || ((side === 'next' || side === 'prev') && tree.contains(doc.activeElement))
      if (show) { root.classList.add('m-files-show-content'); if (detailOn) issueRoot.focus(); else if (diffOn) diff.focus(); else content.focus() } else { root.classList.remove('m-files-show-content'); tree.focus() }
      return true
    }, destroy() { alive = false; clearTimeout(timer); diffEpoch++; dropPatches(); diff.clear(); split.destroy(); inner.destroy(); issueView.destroy?.() },
  }
}
