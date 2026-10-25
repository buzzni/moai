// Wiki tab: the page list on the left, the rendered page on the right (design.md 4.10, tui-analysis.md 10).
// Standalone: pages come in through setPages(), page bodies through props.loadPage(), and the view keeps its
// own state (current page, trail, search, selected link, raw toggle) — design.md 1.2. state()/restore() let the
// shell carry that state across tab switches. Keys go through handleKey(); a keydown listener on the root
// calls it too unless props.standaloneKeys is false. All text goes in through textContent.

import { COPY, fill, type WikiCopy } from "../i18n/index.ts"
import { banner, button, chip, el, emptyState, listRow, searchBox, type ChipTone } from "./components.ts"
import { githubAnchor, renderMarkdown, type LinkTarget } from "./markdown.ts"
import { FORCE_BOTTOM_BELOW } from "./responsive.ts"
import { createSplit, normalizeRatio } from "./split.ts"
import type { IssueIndex } from "../data/index.ts"
import type { MountPane, PaneView } from "./types.ts"

// ---- Data shapes (`moai wiki ls --json` / `moai wiki show <slug> --json`) ----

export interface WikiIssueRef {
  id: string
  exists?: boolean
}

export interface WikiLink {
  text: string
  to: string
  resolved?: boolean
  anchor?: string
}

export interface WikiProblem {
  kind: string
  said: string
}

export interface WikiPage {
  slug: string
  title: string
  path: string
  bytes?: number
  issues: WikiIssueRef[]
  links: WikiLink[]
  linked_from: string[]
  /** `show` only: true when some page could not be read, so linked_from may be short. */
  linked_from_partial?: boolean
  conflict: boolean
  /** Set when the body could not be read (too_large, refused, failed). */
  error?: WikiProblem
  headings?: unknown[]
  /** `show` only. */
  body?: string
}

export interface WikiSkipped {
  path: string
  kind: string
  said: string
}

export interface WikiListing {
  dir: string
  pages: WikiPage[]
  skipped: WikiSkipped[]
}

export type Parsed<T> = { ok: true; value: T } | { ok: false; error: string }

/** Pages over this many bytes are listed but not read (moai `wiki::TOO_LARGE`). */
export const TOO_LARGE = 1024 * 1024

const HOMES = ["readme", "index"]

// ---- Parsers ----

const isObject = (value: unknown): value is Record<string, unknown> => typeof value === "object" && value !== null && !Array.isArray(value)
const str = (value: unknown): string | undefined => (typeof value === "string" ? value : undefined)

/** The words when a caller passes none. */
const DEFAULT_WORDS: WikiCopy = COPY.ko.wiki

function decodeJson(input: unknown, words: WikiCopy): Parsed<unknown> {
  if (typeof input !== "string") return { ok: true, value: input }
  try {
    return { ok: true, value: JSON.parse(input) }
  } catch (error) {
    return { ok: false, error: fill(words.badJson, { reason: (error as Error).message }) }
  }
}

function problem(value: unknown): WikiProblem | undefined {
  if (typeof value === "string") return value ? { kind: "failed", said: value } : undefined
  if (!isObject(value)) return undefined
  const kind = str(value.kind) ?? "failed"
  return { kind, said: str(value.said) ?? str(value.message) ?? kind }
}

/** One page object to a WikiPage, or null when it has no slug. Everything beyond the slug is optional. */
export function normalizePage(value: unknown): WikiPage | null {
  if (!isObject(value)) return null
  const slug = str(value.slug)
  if (!slug) return null
  const issues: WikiIssueRef[] = []
  if (Array.isArray(value.issues)) {
    for (const item of value.issues) {
      if (typeof item === "string") issues.push({ id: item })
      else if (isObject(item) && str(item.id)) issues.push({ id: item.id as string, exists: typeof item.exists === "boolean" ? item.exists : undefined })
    }
  }
  const links: WikiLink[] = []
  if (Array.isArray(value.links)) {
    for (const item of value.links) {
      if (!isObject(item) || !str(item.to)) continue
      links.push({ text: str(item.text) ?? "", to: item.to as string, resolved: typeof item.resolved === "boolean" ? item.resolved : undefined, anchor: str(item.anchor) })
    }
  }
  const page: WikiPage = {
    slug,
    title: str(value.title) || slug,
    path: str(value.path) ?? "",
    issues,
    links,
    linked_from: Array.isArray(value.linked_from) ? value.linked_from.filter((s): s is string => typeof s === "string") : [],
    conflict: value.conflict === true,
  }
  if (typeof value.bytes === "number" && Number.isFinite(value.bytes)) page.bytes = value.bytes
  if (value.linked_from_partial === true) page.linked_from_partial = true
  const error = problem(value.error)
  if (error) page.error = error
  if (Array.isArray(value.headings)) page.headings = value.headings
  if (typeof value.body === "string") page.body = value.body
  return page
}

/** `moai wiki ls --json` (text or parsed): `{dir, pages, skipped?}`, or `{dir, error}` when the folder cannot be used. */
export function parseWikiLs(input: unknown, words: WikiCopy = DEFAULT_WORDS): Parsed<WikiListing> {
  const decoded = decodeJson(input, words)
  if (!decoded.ok) return decoded
  const json = decoded.value
  if (!isObject(json)) return { ok: false, error: words.listShape }
  const dir = str(json.dir) ?? ""
  if (json.error !== undefined && !Array.isArray(json.pages)) {
    return { ok: false, error: problem(json.error)?.said ?? words.folderFailed }
  }
  if (!Array.isArray(json.pages)) return { ok: false, error: words.listNoPages }
  const pages = json.pages.map(normalizePage).filter((p): p is WikiPage => p !== null)
  const skipped: WikiSkipped[] = Array.isArray(json.skipped)
    ? json.skipped.filter(isObject).map((s) => ({ path: str(s.path) ?? "", kind: str(s.kind) ?? "", said: str(s.said) ?? "" }))
    : []
  return { ok: true, value: { dir, pages, skipped } }
}

/**
 * `moai wiki search --json` (text or parsed): the slugs its hits stand on, best first, each once (a page's best
 * section places it). Null when the answer is not that shape — the list then keeps the plain substring order.
 */
export function parseWikiSearch(input: unknown, words: WikiCopy = DEFAULT_WORDS): string[] | null {
  const decoded = decodeJson(input, words)
  if (!decoded.ok || !isObject(decoded.value) || !Array.isArray(decoded.value.hits)) return null
  const out: string[] = []
  for (const hit of decoded.value.hits) {
    const slug = isObject(hit) ? str(hit.slug) : undefined
    if (slug !== undefined && !out.includes(slug)) out.push(slug)
  }
  return out
}

/** `moai wiki show <slug> --json` (text or parsed): the page fields plus `body`. */
export function parseWikiShow(input: unknown, words: WikiCopy = DEFAULT_WORDS): Parsed<WikiPage> {
  const decoded = decodeJson(input, words)
  if (!decoded.ok) return decoded
  const json = decoded.value
  if (isObject(json) && json.error !== undefined && !str(json.slug)) {
    return { ok: false, error: problem(json.error)?.said ?? words.pageFailed }
  }
  const page = normalizePage(json)
  return page ? { ok: true, value: page } : { ok: false, error: words.pageShape }
}

// ---- Ordering, warnings, search ----

const basename = (slug: string): string => slug.slice(slug.lastIndexOf("/") + 1)

/** Where a slug sorts among the homes: the root's README, index, then a folder's; every other page after them. */
export function homeRank(slug: string): number {
  const home = HOMES.indexOf(basename(slug).toLowerCase())
  if (home < 0) return HOMES.length * 2
  return slug.includes("/") ? HOMES.length + home : home
}

/**
 * Code-point order, as Rust compares `str` (moai's sort). Plain `<` compares UTF-16 units, which puts astral
 * characters (emoji, CJK extension B) before U+E000-U+FFFF; shifting the units fixes that.
 */
function compareCodePoints(a: string, b: string): number {
  const n = Math.min(a.length, b.length)
  for (let i = 0; i < n; i++) {
    let x = a.charCodeAt(i)
    let y = b.charCodeAt(i)
    if (x === y) continue
    if (x >= 0xd800) x += x < 0xe000 ? 0x2000 : -0x800
    if (y >= 0xd800) y += y < 0xe000 ? 0x2000 : -0x800
    return x - y
  }
  return a.length - b.length
}

/** README and index first (the wiki root's before a folder's), then by lowercased title, then by slug. Stable. */
export function sortPages<T extends { slug: string; title?: string }>(pages: ReadonlyArray<T>): T[] {
  return pages
    .map((page, index) => ({ page, index }))
    .sort((a, b) => {
      const rank = homeRank(a.page.slug) - homeRank(b.page.slug)
      if (rank) return rank
      // Code-point order of the lowercased titles, as moai sorts (not localeCompare, which follows the host locale).
      const ta = (a.page.title || a.page.slug).toLowerCase()
      const tb = (b.page.title || b.page.slug).toLowerCase()
      return compareCodePoints(ta, tb) || compareCodePoints(a.page.slug, b.page.slug) || a.index - b.index
    })
    .map((entry) => entry.page)
}

export interface PageWarning {
  kind: "conflict" | "error" | "large"
  label: string
  tone: ChipTone
  /** The longer explanation, for the chip's title. */
  detail: string
}

/** Warning chips for a page: git conflict markers, an unreadable body, more than 1 MB. */
export function pageWarnings(page: Pick<WikiPage, "conflict" | "error" | "bytes">, words: WikiCopy = DEFAULT_WORDS): PageWarning[] {
  const out: PageWarning[] = []
  if (page.conflict) out.push({ kind: "conflict", label: words.conflict, tone: "danger", detail: words.conflictDetail })
  const large = (page.bytes ?? 0) > TOO_LARGE || page.error?.kind === "too_large"
  if (page.error && page.error.kind !== "too_large") out.push({ kind: "error", label: words.unread, tone: "danger", detail: page.error.said })
  if (large) out.push({ kind: "large", label: words.large, tone: "warn", detail: page.error?.kind === "too_large" ? page.error.said : words.largeDetail })
  return out
}

export type BodySource = ReadonlyMap<string, string> | Readonly<Record<string, string>>

function bodyOf(bodies: BodySource | undefined, page: { slug: string; body?: string }): string | undefined {
  if (typeof page.body === "string") return page.body
  if (!bodies) return undefined
  if (typeof (bodies as ReadonlyMap<string, string>).get === "function") return (bodies as ReadonlyMap<string, string>).get(page.slug)
  const value = (bodies as Record<string, string>)[page.slug]
  return typeof value === "string" ? value : undefined
}

/** Case-insensitive substring over the title, and over the body when it is known (loaded or passed in). */
export function filterPages<T extends { slug: string; title: string; body?: string }>(pages: ReadonlyArray<T>, query: string, bodies?: BodySource): T[] {
  const q = query.trim().toLowerCase()
  if (!q) return pages.slice()
  return pages.filter((page) => {
    if (page.title.toLowerCase().includes(q)) return true
    const body = bodyOf(bodies, page)
    return body !== undefined && body.toLowerCase().includes(q)
  })
}

/**
 * The list for a query (moai-xp7l.nyi): the pages `moai wiki search` ranked, in its order, then the pages
 * {@link filterPages} finds that it did not rank, in list order. The rank is moai's own (the CLI and the explorer
 * use the same one), so the three surfaces agree; the substring half keeps the list narrowing while a word is half
 * typed (`gui` ranks nothing, yet finds `guide`). Without a rank for this query it is the substring list alone.
 */
export function rankPages<T extends { slug: string; title: string; body?: string }>(
  pages: ReadonlyArray<T>,
  query: string,
  bodies?: BodySource,
  ranked?: ReadonlyArray<string> | null,
): T[] {
  const found = filterPages(pages, query, bodies)
  if (!query.trim() || !ranked?.length) return found
  const bySlug = new Map(pages.map((page) => [page.slug, page] as const))
  const out: T[] = []
  const seen = new Set<string>()
  for (const slug of ranked) {
    const page = bySlug.get(slug)
    if (page && !seen.has(slug)) {
      seen.add(slug)
      out.push(page)
    }
  }
  for (const page of found) if (!seen.has(page.slug)) out.push(page)
  return out
}

/** How long the search field waits after the last key before it asks moai for the rank, in ms. */
export const RANK_DELAY_MS = 150

// ---- Link targets (port of moai `wiki::target`) ----

/** `%XX` to bytes, read as UTF-8; anything malformed or not UTF-8 comes back as given. */
export function unpercent(text: string): string {
  if (!text.includes("%")) return text
  const bytes: number[] = []
  const raw = new TextEncoder().encode(text)
  const hex = (c: number | undefined): number => {
    if (c === undefined) return -1
    if (c >= 48 && c <= 57) return c - 48
    if (c >= 65 && c <= 70) return c - 55
    if (c >= 97 && c <= 102) return c - 87
    return -1
  }
  for (let i = 0; i < raw.length; i++) {
    const h = raw[i] === 37 ? hex(raw[i + 1]) : -1
    const l = h >= 0 ? hex(raw[i + 2]) : -1
    if (h >= 0 && l >= 0) {
      bytes.push(h * 16 + l)
      i += 2
    } else bytes.push(raw[i]!)
  }
  try {
    // ignoreBOM keeps a decoded leading U+FEFF, as Rust's String::from_utf8 does (the decoder drops it by default).
    return new TextDecoder("utf-8", { fatal: true, ignoreBOM: true }).decode(new Uint8Array(bytes))
  } catch {
    return text
  }
}

/**
 * What an href written in page `fromSlug` points at. Only relative `.md` paths landing inside the wiki are
 * wiki links, resolved against the folder of the page they are written in; `?query` is dropped, `%XX` decoded,
 * and `#anchor` split off (decoded, GitHub-slugged). `#x` alone is a heading on this page. Absolute paths,
 * `scheme:` addresses, other files and paths climbing out of the wiki root are external. With `knownSlugs`, a
 * wiki link to a page that is not there is external too, so it is shown and never followed.
 */
export function resolveWikiTarget(fromSlug: string, href: string, knownSlugs?: Iterable<string>): LinkTarget {
  const external: LinkTarget = { kind: "external", href }
  const hash = href.indexOf("#")
  const head = hash < 0 ? href : href.slice(0, hash)
  const fragment = hash < 0 ? "" : unpercent(href.slice(hash + 1))
  const anchor = fragment ? githubAnchor(fragment) : ""
  if (!head) return anchor ? { kind: "anchor", anchor } : external
  if (head.startsWith("/") || head.startsWith("\\")) return external
  if (head.split("/")[0]!.includes(":")) return external
  const path = unpercent(head.split("?")[0]!)
  if (!path.endsWith(".md")) return external
  const segs = path.slice(0, -3).split("/")
  const last = segs[segs.length - 1]
  if (last === undefined || last === "" || last === "." || last === "..") return external
  const parts = fromSlug.split("/")
  parts.pop()
  for (const seg of segs) {
    if (seg === "" || seg === ".") continue
    if (seg === "..") {
      if (!parts.length) return external
      parts.pop()
    } else parts.push(seg)
  }
  const slug = parts.join("/")
  if (knownSlugs) {
    const known = knownSlugs instanceof Set ? (knownSlugs as Set<string>) : new Set(knownSlugs)
    if (!known.has(slug)) return external
  }
  return anchor ? { kind: "wiki", slug, anchor } : { kind: "wiki", slug }
}

// ---- Trail ----

export interface TrailEntry {
  slug: string
  scrollTop: number
}

export const TRAIL_LIMIT = 100

/** The trail with `entry` on top; the oldest entries fall off past `limit`. */
export function pushTrail(trail: ReadonlyArray<TrailEntry>, entry: TrailEntry, limit = TRAIL_LIMIT): TrailEntry[] {
  const next = [...trail, { slug: entry.slug, scrollTop: Math.max(0, entry.scrollTop || 0) }]
  return next.length > limit ? next.slice(next.length - limit) : next
}

/** The top entry and the trail without it, or null when the trail is empty. */
export function popTrail(trail: ReadonlyArray<TrailEntry>): { entry: TrailEntry; trail: TrailEntry[] } | null {
  if (!trail.length) return null
  return { entry: trail[trail.length - 1]!, trail: trail.slice(0, -1) }
}

/** The next selected link for Tab (+1) or Shift-Tab (-1) among `count` links; wraps. -1 means none. */
export function cycleIndex(current: number, delta: 1 | -1, count: number): number {
  if (count <= 0) return -1
  if (current < 0 || current >= count) return delta > 0 ? 0 : count - 1
  return (current + delta + count) % count
}

// ---- View ----

export type WikiPane = "list" | "body"

export interface WikiState {
  /** The page shown on the right. */
  current: string | null
  /** The list cursor. */
  cursor: string | null
  trail: TrailEntry[]
  query: string
  raw: boolean
  scrollTop: number
  selectedLink: number
  pane: WikiPane
}

export interface WikiViewProps {
  /** Fetch one page (`moai wiki show <slug> --json`, parsed or not). */
  loadPage: (slug: string) => Promise<unknown>
  /** Rank the pages for a query (`moai wiki search <query> --json`, parsed or not); without it the list is a substring filter. */
  searchPages?: (query: string) => Promise<unknown>
  /** An issue id link was followed: the shell jumps to that row on the list tab. */
  onIssue?: (id: string) => void
  /** The list's share of the width (the `wiki_width` setting) after a drag or key press. */
  onRatio?: (ratio: number) => void
  ratio?: number
  width?: number
  pages?: WikiPage[]
  state?: Partial<WikiState>
  /** Attach a keydown listener on the root that calls handleKey (default true). */
  standaloneKeys?: boolean
  /** How long a heading reached by an anchor stays highlighted, in ms. */
  flashMs?: number
  /** The words (copy.wiki); Korean when not given. */
  words?: WikiCopy
}

export interface WikiView {
  el: HTMLDivElement
  setPages(pages: ReadonlyArray<WikiPage>, dir?: string): void
  setError(text: string | null): void
  setLoading(loading: boolean): void
  /** Seed a body for search (e.g. from a prefetch). */
  setBody(slug: string, body: string): void
  /** Show a page. push=true records the current page and scroll on the trail first. */
  open(slug: string, options?: { anchor?: string; push?: boolean; scrollTop?: number }): Promise<void>
  back(): Promise<boolean>
  selectLink(delta: 1 | -1): void
  clearLinkSelection(): boolean
  followSelected(): Promise<void>
  follow(target: LinkTarget | { kind: "issue"; id: string }): Promise<void>
  scrollToAnchor(anchor: string): boolean
  toggleRaw(): void
  setQuery(query: string): void
  focusSearch(): void
  focusPane(pane: WikiPane): void
  handleKey(event: KeyboardEvent): boolean
  /**
   * A key-table action by its id (keys/wiring.ts), for what handleKey cannot read off the event: `move.top` reached
   * by `gg`, the page moves, the `SPC w` / `Ctrl-w` focus moves, a letter a Korean input source turned into a
   * jamo. True when the view did it; `wiki.clear` with nothing to clear is false.
   */
  command(action: string): boolean
  setWidth(width: number): void
  setRatio(ratio: number): void
  state(): WikiState
  restore(state: Partial<WikiState>): Promise<void>
  /** Links in the body, in Tab order (rendered links, then backlinks). */
  links(): HTMLAnchorElement[]
  destroy(): void
}

const DEFAULT_LIST_SHARE = 0.3
const DEFAULT_FLASH_MS = 1500
const LINE_PX = 40

function isTextInput(target: EventTarget | null): boolean {
  if (!target || typeof (target as Element).tagName !== "string") return false
  const node = target as HTMLElement
  const tag = node.tagName.toLowerCase()
  return tag === "input" || tag === "textarea" || tag === "select" || node.isContentEditable === true || node.getAttribute?.("contenteditable") === "true"
}

function isButton(target: EventTarget | null): boolean {
  return !!target && typeof (target as Element).tagName === "string" && (target as Element).tagName.toLowerCase() === "button"
}

const folderOf = (slug: string): string => (slug.includes("/") ? slug.slice(0, slug.lastIndexOf("/") + 1) : "")

export function createWikiView(doc: Document, props: WikiViewProps): WikiView {
  const flashMs = props.flashMs ?? DEFAULT_FLASH_MS
  const words = props.words ?? DEFAULT_WORDS
  let pages: WikiPage[] = []
  let dir = ""
  let listError: string | null = null
  let listLoading = false
  const bodies = new Map<string, string>()

  let current: string | null = null
  let page: WikiPage | null = null
  let pageLoading = false
  let pageError: string | null = null
  let cursor: string | null = null
  let trail: TrailEntry[] = []
  let query = ""
  /** The rank moai gave the query it was asked for — used only while that is still the query. */
  let ranked: { query: string; slugs: string[] } | null = null
  let rankTimer: ReturnType<typeof setTimeout> | null = null
  let raw = false
  let selected = -1
  let pane: WikiPane = "list"
  let links: HTMLAnchorElement[] = []
  let headingNodes = new Map<string, HTMLElement>()
  let request = 0
  let flashTimer: ReturnType<typeof setTimeout> | null = null

  // -- skeleton --
  const search = searchBox(doc, { placeholder: words.searchPlaceholder, label: words.searchLabel, hint: "/" })
  search.el.classList.add("m-wiki-search")
  const toolbar = el(doc, "div", { class: "m-toolbar m-wiki-toolbar" }, search.el)

  const listBox = el(doc, "div", { class: "m-wiki-list", role: "listbox", "aria-label": words.pagesLabel })
  const listNote = el(doc, "div", { class: "m-wiki-list-note" })
  const listPane = el(doc, "div", { class: "m-wiki-pages" }, listNote, listBox)

  const titleEl = el(doc, "h2", { class: "m-wiki-title" })
  const pathEl = el(doc, "div", { class: "m-wiki-path" })
  const backButton = button(doc, { label: words.back, icon: "corner-left-up", variant: "ghost", small: true, onClick: () => void back() })
  const rawButton = button(doc, { label: words.raw, variant: "ghost", small: true, onClick: () => toggleRaw() })
  rawButton.setAttribute("aria-pressed", "false")
  rawButton.title = words.rawTitle
  const head = el(doc, "div", { class: "m-wiki-head" }, el(doc, "div", { class: "m-wiki-heading" }, titleEl, pathEl), el(doc, "div", { class: "m-wiki-actions" }, backButton, rawButton))
  const status = el(doc, "div", { class: "m-wiki-status", role: "status", "aria-live": "polite" })
  const scroller = el(doc, "div", { class: "m-wiki-scroll", tabindex: 0, "aria-label": words.bodyLabel })
  const bodyPane = el(doc, "div", { class: "m-wiki-body" }, head, status, scroller)

  const split = createSplit(doc, {
    position: "left",
    ratios: { left: props.ratio === undefined ? DEFAULT_LIST_SHARE : normalizeRatio(props.ratio, "left") },
    labels: { resizer: words.resizer },
    onRatio: (position, ratio) => {
      if (position === "left") props.onRatio?.(ratio)
    },
  })
  // The split's "detail" pane is the one drawn first (left), so it holds the page list; its ratio is wiki_width.
  split.detail.append(listPane)
  split.list.append(bodyPane)
  const root = el(doc, "div", { class: "m-wiki", "data-pane": pane }, toolbar, split.el)

  // -- list --
  const visiblePages = (): WikiPage[] =>
    rankPages(pages, query, bodies, ranked && ranked.query === query.trim() ? ranked.slugs : null)

  /**
   * Ask moai for the rank of the query, once the keys stop. An answer for a query that is no longer typed is
   * dropped; a failed one (a query with no words in it, moai out of reach) leaves the substring list as it was.
   */
  function askRank(): void {
    if (rankTimer) clearTimeout(rankTimer)
    rankTimer = null
    const asked = query.trim()
    if (!props.searchPages || !asked || ranked?.query === asked) return
    rankTimer = setTimeout(() => {
      rankTimer = null
      props.searchPages!(asked).then(
        (answer) => {
          const slugs = parseWikiSearch(answer, words)
          if (!slugs || query.trim() !== asked) return
          ranked = { query: asked, slugs }
          paintList()
        },
        () => {},
      )
    }, RANK_DELAY_MS)
  }
  const titleOf = (slug: string): string => pages.find((p) => p.slug === slug)?.title ?? slug

  /**
   * Repaint the list. Rebuilding drops the focused row, which would leave focus on <body> — outside the root, so
   * its keydown listener stops hearing j/k/Enter — so focus goes back to the cursor row (or the page).
   */
  function paintList(): void {
    const had = listBox.contains(doc.activeElement)
    buildList()
    if (had && !listBox.contains(doc.activeElement)) {
      focusCursorRow()
      if (!root.contains(doc.activeElement)) scroller.focus({ preventScroll: true })
    }
  }

  function buildList(): void {
    listNote.replaceChildren()
    listBox.replaceChildren()
    if (listError) {
      listNote.append(banner(doc, "danger", listError))
      return
    }
    if (listLoading && !pages.length) {
      listNote.append(emptyState(doc, { icon: "book-open", title: words.loadingList }))
      return
    }
    if (!pages.length) {
      listNote.append(emptyState(doc, { icon: "book-open", title: words.noPages, hint: dir ? fill(words.noPagesHint, { dir }) : undefined }))
      return
    }
    const shown = visiblePages()
    if (!shown.length) {
      listNote.append(emptyState(doc, { icon: "search", title: words.noMatch, hint: words.noMatchHint }))
      return
    }
    if (!cursor || !shown.some((p) => p.slug === cursor)) cursor = shown.some((p) => p.slug === current) ? current : shown[0]!.slug
    for (const entry of shown) {
      const cells: Node[] = [el(doc, "span", { class: "m-row-title", title: entry.title }, entry.title)]
      for (const warning of pageWarnings(entry, words)) {
        const node = chip(doc, { label: warning.label, tone: warning.tone })
        node.title = warning.detail
        cells.push(node)
      }
      const folder = folderOf(entry.slug)
      if (folder) cells.push(el(doc, "span", { class: "m-row-meta m-wiki-folder" }, folder))
      const row = listRow(doc, { selected: entry.slug === cursor }, ...cells)
      row.dataset.slug = entry.slug
      if (entry.slug === current) row.setAttribute("aria-current", "page")
      row.addEventListener("click", () => {
        cursor = entry.slug
        pane = "list"
        void open(entry.slug)
      })
      listBox.append(row)
    }
  }

  function moveCursor(delta: number): void {
    const shown = visiblePages()
    if (!shown.length) return
    const index = shown.findIndex((p) => p.slug === cursor)
    const next = Math.max(0, Math.min(shown.length - 1, (index < 0 ? 0 : index + delta)))
    cursor = shown[next]!.slug
    paintList()
    focusCursorRow()
  }

  /** Rows the page list shows at once, less one kept for context; 10 when it has not been laid out (no height). */
  function listPageRows(): number {
    const rowHeight = (listBox.firstElementChild as HTMLElement | null)?.offsetHeight ?? 0
    const height = listPane.clientHeight
    if (!rowHeight || !height) return 10
    return Math.max(1, Math.floor(height / rowHeight) - 1)
  }

  function focusCursorRow(): void {
    const row = listBox.querySelector<HTMLElement>('[aria-selected="true"]')
    row?.focus({ preventScroll: true })
    row?.scrollIntoView?.({ block: "nearest" })
  }

  // -- body --
  function setStatus(text: string): void {
    status.textContent = text
  }

  function backlinksSection(source: WikiPage): HTMLElement {
    const section = el(doc, "section", { class: "m-wiki-backlinks", "aria-label": words.backlinksLabel })
    section.append(el(doc, "div", { class: "m-wiki-backlinks-title" }, fill(words.backlinks, { n: source.linked_from.length })))
    if (!source.linked_from.length) {
      section.append(el(doc, "div", { class: "m-wiki-backlinks-none" }, words.noBacklinks))
    } else {
      const list = el(doc, "ul", { class: "m-wiki-backlinks-list" })
      for (const slug of source.linked_from) {
        const link = el(doc, "a", { class: "m-md-link m-wiki-backlink", tabindex: -1, "data-kind": "wiki", "data-slug": slug }, titleOf(slug))
        list.append(el(doc, "li", {}, link, el(doc, "span", { class: "m-wiki-backlink-slug" }, slug)))
      }
      section.append(list)
    }
    if (source.linked_from_partial) section.append(el(doc, "div", { class: "m-wiki-backlinks-none" }, words.backlinksPartial))
    return section
  }

  /**
   * Repaint the page. Like paintList, a rebuild drops a focused link, and the back button disables itself once
   * the trail is empty (the browser then moves focus to <body>); either way focus goes to the page.
   */
  function paintBody(): void {
    const active = doc.activeElement
    const had = active !== scroller && scroller.contains(active)
    buildBody()
    const lost = had && !scroller.contains(doc.activeElement)
    if (lost || (doc.activeElement === backButton && backButton.disabled)) scroller.focus({ preventScroll: true })
  }

  function buildBody(): void {
    selected = -1
    links = []
    headingNodes = new Map()
    scroller.replaceChildren()
    const listed = current ? pages.find((p) => p.slug === current) : undefined
    const shown = page ?? listed ?? null
    titleEl.textContent = shown?.title ?? (current ?? "")
    pathEl.textContent = shown?.path ?? ""
    backButton.disabled = trail.length === 0
    rawButton.setAttribute("aria-pressed", String(raw))
    if (!current) {
      scroller.append(emptyState(doc, { icon: "book-open", title: words.choosePage, hint: words.choosePageHint }))
      return
    }
    if (pageLoading) {
      scroller.append(emptyState(doc, { icon: "book-open", title: words.loadingPage }))
      return
    }
    if (pageError) {
      scroller.append(banner(doc, "danger", pageError))
      return
    }
    if (!page) return
    if (page.conflict) scroller.append(banner(doc, "warn", words.pageConflict))
    if (typeof page.body !== "string") {
      scroller.append(banner(doc, page.error?.kind === "too_large" ? "warn" : "danger", page.error?.said ?? words.bodyFailed))
    } else if (raw) {
      scroller.append(el(doc, "pre", { class: "m-wiki-raw" }, page.body))
    } else {
      const from = page.slug
      const known = pages.length ? new Set(pages.map((p) => p.slug)) : undefined
      try {
        const rendered = renderMarkdown(doc, page.body, {
          resolveLink: (href) => resolveWikiTarget(from, href, known),
          // An id moai found no issue for (exists: false) stays text: there is no row to jump to.
          issueIds: page.issues.filter((issue) => issue.exists !== false).map((issue) => issue.id),
        })
        rendered.root.classList.add("m-wiki-article")
        scroller.append(rendered.root)
        links.push(...rendered.links)
        for (const heading of rendered.headings) {
          const node = rendered.root.querySelector<HTMLElement>(`#${cssEscape("m-md-" + heading.anchor)}`)
          if (node && !headingNodes.has(heading.anchor)) headingNodes.set(heading.anchor, node)
        }
      } catch (error) {
        scroller.append(banner(doc, "warn", fill(words.renderFailed, { reason: (error as Error).message })))
        scroller.append(el(doc, "pre", { class: "m-wiki-raw" }, page.body))
      }
    }
    const backlinks = backlinksSection(page)
    scroller.append(backlinks)
    links.push(...backlinks.querySelectorAll<HTMLAnchorElement>("a.m-md-link"))
  }

  function cssEscape(value: string): string {
    const css = (doc.defaultView as (Window & { CSS?: { escape?: (s: string) => string } }) | null)?.CSS
    if (css?.escape) return css.escape(value)
    return value.replace(/[^a-zA-Z0-9_ -￿-]/g, (c) => `\\${c}`)
  }

  function headingFor(anchor: string): HTMLElement | null {
    const known = headingNodes.get(anchor)
    if (known) return known
    for (const node of scroller.querySelectorAll<HTMLElement>("[data-anchor]")) {
      if (node.dataset.anchor === anchor && !node.classList.contains("m-md-link")) return node
    }
    return null
  }

  function scrollToAnchor(anchor: string): boolean {
    const heading = headingFor(anchor)
    if (!heading) {
      // The raw view has no heading elements; saying the heading is missing would be wrong there.
      setStatus(raw ? fill(words.rawNoAnchor, { anchor }) : fill(words.noAnchor, { anchor }))
      return false
    }
    const top = heading.getBoundingClientRect().top - scroller.getBoundingClientRect().top
    scroller.scrollTop = Math.max(0, scroller.scrollTop + top)
    for (const node of scroller.querySelectorAll(".m-wiki-flash")) node.classList.remove("m-wiki-flash")
    heading.classList.add("m-wiki-flash")
    if (flashTimer) clearTimeout(flashTimer)
    flashTimer = setTimeout(() => {
      heading.classList.remove("m-wiki-flash")
      flashTimer = null
    }, flashMs)
    return true
  }

  /** Mark the selected link (roving tabindex); with `focus`, also focus it and scroll it into view. */
  function paintSelection(focus = true): void {
    links.forEach((link, index) => {
      const on = index === selected
      link.tabIndex = on ? 0 : -1
      link.classList.toggle("m-wiki-link-selected", on)
      if (on) link.setAttribute("aria-current", "true")
      else link.removeAttribute("aria-current")
    })
    const link = links[selected]
    if (link && focus) {
      link.focus({ preventScroll: true })
      link.scrollIntoView?.({ block: "nearest" })
    }
  }

  function selectLink(delta: 1 | -1): void {
    selected = cycleIndex(selected, delta, links.length)
    pane = "body"
    root.dataset.pane = pane
    paintSelection()
    if (!links.length) setStatus(words.noLinks)
  }

  function clearLinkSelection(): boolean {
    if (selected < 0) return false
    selected = -1
    paintSelection()
    scroller.focus({ preventScroll: true })
    return true
  }

  function targetOf(link: HTMLAnchorElement): LinkTarget | { kind: "issue"; id: string } {
    const data = link.dataset
    if (data.kind === "issue") return { kind: "issue", id: data.issue ?? link.textContent ?? "" }
    if (data.kind === "wiki" && data.slug) return data.anchor ? { kind: "wiki", slug: data.slug, anchor: data.anchor } : { kind: "wiki", slug: data.slug }
    if (data.kind === "anchor" && data.anchor) return { kind: "anchor", anchor: data.anchor }
    return { kind: "external", href: data.href ?? link.title ?? "" }
  }

  async function follow(target: LinkTarget | { kind: "issue"; id: string }): Promise<void> {
    switch (target.kind) {
      case "issue":
        if (target.id) props.onIssue?.(target.id)
        return
      case "anchor":
        scrollToAnchor(target.anchor)
        return
      case "external":
        // Shown, never opened: the panel CSP blocks navigation and the renderer gives links no href.
        setStatus(fill(words.outside, { href: target.href }))
        return
      case "wiki":
        if (target.slug === current && page) {
          if (target.anchor) scrollToAnchor(target.anchor)
          else scroller.scrollTop = 0
          return
        }
        await open(target.slug, { anchor: target.anchor, push: true })
    }
  }

  async function followSelected(): Promise<void> {
    const link = links[selected]
    if (link) await follow(targetOf(link))
  }

  // -- navigation --
  async function open(slug: string, options: { anchor?: string; push?: boolean; scrollTop?: number } = {}): Promise<void> {
    if (options.push && current && current !== slug) trail = pushTrail(trail, { slug: current, scrollTop: scroller.scrollTop })
    const token = ++request
    current = slug
    cursor = slug
    page = null
    pageError = null
    pageLoading = true
    setStatus("")
    paintList()
    paintBody()
    let next: WikiPage | null = null
    let error: string | null = null
    try {
      const parsed = parseWikiShow(await props.loadPage(slug), words)
      if (parsed.ok) next = parsed.value
      else error = parsed.error
    } catch (reason) {
      error = fill(words.loadFailed, { reason: reason instanceof Error ? reason.message : String(reason) })
    }
    if (token !== request) return // a later open won; this answer is stale
    pageLoading = false
    page = next
    pageError = error
    if (next && typeof next.body === "string") bodies.set(next.slug, next.body)
    paintList()
    paintBody()
    scroller.scrollTop = options.scrollTop ?? 0
    if (options.anchor && next) scrollToAnchor(options.anchor)
  }

  async function back(): Promise<boolean> {
    const popped = popTrail(trail)
    if (!popped) {
      setStatus(words.nothingBack)
      return false
    }
    trail = popped.trail
    await open(popped.entry.slug, { scrollTop: popped.entry.scrollTop })
    return true
  }

  function toggleRaw(): void {
    const top = scroller.scrollTop
    raw = !raw
    paintBody()
    scroller.scrollTop = top
  }

  function setQuery(next: string): void {
    query = next
    if (search.input.value !== next) search.input.value = next
    root.dataset.searching = next.trim() ? "true" : "false"
    paintList()
    askRank()
  }

  function focusSearch(): void {
    search.input.focus()
    search.input.select?.()
  }

  function focusPane(next: WikiPane): void {
    pane = next
    root.dataset.pane = pane
    if (pane === "list") focusCursorRow()
    else if (links[selected]) links[selected]!.focus({ preventScroll: true })
    else scroller.focus({ preventScroll: true })
  }

  // -- keys --
  function handleKey(event: KeyboardEvent): boolean {
    if (event.isComposing || event.defaultPrevented) return false
    const done = (): boolean => {
      event.preventDefault()
      return true
    }
    if (event.target === search.input) {
      if (event.key === "Escape") {
        if (query) setQuery("")
        focusPane("list")
        return done()
      }
      if (event.key === "Enter" || event.key === "ArrowDown") {
        focusPane("list")
        return done()
      }
      return false
    }
    if (isTextInput(event.target)) return false
    if (event.ctrlKey || event.metaKey || event.altKey) return false
    // A focused button (back, raw) keeps its own Enter: preventing it would cancel the button's click.
    if (event.key === "Enter" && isButton(event.target)) return false
    switch (event.key) {
      case "/":
        focusSearch()
        return done()
      case "Escape":
        if (clearLinkSelection()) return done()
        if (query) {
          setQuery("")
          return done()
        }
        return false
      case "Tab":
        selectLink(event.shiftKey ? -1 : 1)
        return done()
      // Back is the same in both panes, as in the TUI (Bksp, h, ←).
      case "Backspace":
      case "h":
      case "ArrowLeft":
        void back()
        return done()
    }
    if (pane === "list") {
      switch (event.key) {
        case "j":
        case "ArrowDown":
          moveCursor(1)
          return done()
        case "k":
        case "ArrowUp":
          moveCursor(-1)
          return done()
        case "Home":
          moveCursor(-Infinity)
          return done()
        case "End":
          moveCursor(Infinity)
          return done()
        case "Enter":
          if (cursor) void open(cursor)
          return done()
        case "l":
        case "ArrowRight":
          focusPane("body")
          return done()
      }
      return false
    }
    switch (event.key) {
      case "Enter":
        if (selected >= 0) void followSelected()
        else selectLink(1) // the TUI opens a link picker here; the panel selects the first link instead
        return done()
      case "j":
      case "ArrowDown":
        scroller.scrollTop += LINE_PX
        return done()
      case "k":
      case "ArrowUp":
        scroller.scrollTop = Math.max(0, scroller.scrollTop - LINE_PX)
        return done()
    }
    return false
  }

  function command(action: string): boolean {
    const page = Math.max(LINE_PX, scroller.clientHeight - LINE_PX)
    const scrollBy = (by: number) => {
      scroller.scrollTop = Math.max(0, scroller.scrollTop + by)
    }
    switch (action) {
      case "focus.next":
      case "focus.prev":
        focusPane(pane === "list" ? "body" : "list")
        return true
      case "focus.left":
        focusPane("list")
        return true
      case "focus.right":
      case "wiki.page":
        focusPane("body")
        return true
      case "move.down":
      case "move.up":
        if (pane === "list") moveCursor(action === "move.down" ? 1 : -1)
        else scrollBy(action === "move.down" ? LINE_PX : -LINE_PX)
        return true
      case "move.top":
      case "move.bottom":
        if (pane === "list") moveCursor(action === "move.top" ? -Infinity : Infinity)
        else scroller.scrollTop = action === "move.top" ? 0 : scroller.scrollHeight
        return true
      case "move.halfDown":
      case "move.halfUp":
      case "move.pageDown":
      case "move.pageUp": {
        const sign = action.endsWith("Down") ? 1 : -1
        const half = action.startsWith("move.half")
        // In the page list the cursor moves by half a page or a page of rows — on C-d / C-u too, where the issue
        // list leaves its cursor and scrolls (the wiki's help says so: wiki.halfDown); in the page, the page scrolls.
        if (pane === "list") moveCursor(sign * (half ? Math.max(1, Math.floor(listPageRows() / 2)) : listPageRows()))
        else scrollBy(sign * (half ? page / 2 : page))
        return true
      }
      case "wiki.enter":
        if (pane === "list") {
          if (cursor) void open(cursor)
        } else if (selected >= 0) void followSelected()
        else selectLink(1)
        return true
      case "wiki.back":
        void back()
        return true
      case "wiki.nextLink":
      case "wiki.prevLink":
        selectLink(action === "wiki.nextLink" ? 1 : -1)
        return true
      case "wiki.search":
        focusSearch()
        return true
      case "wiki.clear":
        if (clearLinkSelection()) return true
        if (query) {
          setQuery("")
          return true
        }
        return false
    }
    return false
  }

  // -- events --
  const onKeyDown = (event: KeyboardEvent): void => {
    handleKey(event)
  }
  const onInput = (): void => setQuery(search.input.value)
  const onFocusIn = (event: FocusEvent): void => {
    const target = event.target as Node
    if (listPane.contains(target)) pane = "list"
    else if (bodyPane.contains(target)) pane = "body"
    root.dataset.pane = pane
  }
  const onBodyClick = (event: MouseEvent): void => {
    const link = (event.target as Element | null)?.closest?.("a.m-md-link") as HTMLAnchorElement | null
    if (!link || !scroller.contains(link)) return
    event.preventDefault()
    const index = links.indexOf(link)
    if (index >= 0) {
      selected = index
      paintSelection()
    }
    void follow(targetOf(link))
  }
  if (props.standaloneKeys !== false) root.addEventListener("keydown", onKeyDown)
  search.input.addEventListener("input", onInput)
  root.addEventListener("focusin", onFocusIn)
  scroller.addEventListener("click", onBodyClick)

  // -- public --
  function setPages(next: ReadonlyArray<WikiPage>, nextDir?: string): void {
    pages = sortPages(next)
    if (nextDir !== undefined) dir = nextDir
    listError = null
    listLoading = false
    for (const entry of pages) if (typeof entry.body === "string") bodies.set(entry.slug, entry.body)
    if (!current && pages.length) {
      paintList()
      void open(pages[0]!.slug)
      return
    }
    // A refresh re-resolves the body's links against the new page set; keep the reader's place, the selected
    // link and the focus instead of dropping them with the old nodes.
    const keep = selected
    const onLink = keep >= 0 && links[keep] !== undefined && doc.activeElement === links[keep]
    const top = scroller.scrollTop
    paintList()
    paintBody()
    scroller.scrollTop = top
    if (keep >= 0 && keep < links.length) {
      selected = keep
      paintSelection(onLink)
    }
  }

  function setWidth(width: number): void {
    split.setWidth(width)
    root.dataset.narrow = width < FORCE_BOTTOM_BELOW ? "true" : "false"
  }

  function state(): WikiState {
    return { current, cursor, trail: trail.map((e) => ({ ...e })), query, raw, scrollTop: scroller.scrollTop, selectedLink: selected, pane }
  }

  async function restore(saved: Partial<WikiState>): Promise<void> {
    if (Array.isArray(saved.trail)) trail = saved.trail.filter((e) => e && typeof e.slug === "string").map((e) => ({ slug: e.slug, scrollTop: Number(e.scrollTop) || 0 }))
    if (typeof saved.raw === "boolean") raw = saved.raw
    if (saved.pane === "list" || saved.pane === "body") pane = saved.pane
    root.dataset.pane = pane
    if (typeof saved.query === "string") setQuery(saved.query)
    if (typeof saved.cursor === "string") cursor = saved.cursor
    const top = Number(saved.scrollTop) || 0
    if (typeof saved.current === "string" && saved.current) {
      if (saved.current !== current || !page) await open(saved.current, { scrollTop: top })
      else {
        paintBody()
        scroller.scrollTop = top
      }
      if (typeof saved.cursor === "string") cursor = saved.cursor
      paintList()
    } else {
      // No page to show: still paint the list, or the restored cursor would not show until the next repaint.
      paintList()
      paintBody()
    }
    if (typeof saved.selectedLink === "number" && saved.selectedLink >= 0 && saved.selectedLink < links.length) {
      selected = saved.selectedLink
      paintSelection()
    }
  }

  if (props.width !== undefined) setWidth(props.width)
  paintList()
  paintBody()
  if (props.pages) setPages(props.pages)
  if (props.state) void restore(props.state)

  return {
    el: root,
    setPages,
    setError(text) {
      listError = text
      listLoading = false
      paintList()
    },
    setLoading(loading) {
      listLoading = loading
      paintList()
    },
    setBody(slug, body) {
      bodies.set(slug, body)
      if (query) paintList()
    },
    open,
    back,
    selectLink,
    clearLinkSelection,
    followSelected,
    follow,
    scrollToAnchor,
    toggleRaw,
    setQuery,
    focusSearch,
    focusPane,
    handleKey,
    command,
    setWidth,
    setRatio(ratio) {
      split.setRatio("left", ratio)
    },
    state,
    restore,
    links: () => links.slice(),
    destroy() {
      root.removeEventListener("keydown", onKeyDown)
      search.input.removeEventListener("input", onInput)
      root.removeEventListener("focusin", onFocusIn)
      scroller.removeEventListener("click", onBodyClick)
      if (flashTimer) clearTimeout(flashTimer)
      if (rankTimer) clearTimeout(rankTimer)
      split.destroy()
    },
  }
}

// ---- The wiki tab's pane (view/panes.ts) ----

/**
 * The wiki view in the shell's wiki panel (contract: view/types.ts). Pages come from `ctx.wiki` — read the first
 * time the tab shows, and again the next time it shows after a listing that failed (a cancelled read, `moai web`
 * out of reach) — and the view's state is saved into the store (`wiki`) as the tab is left and given back
 * from it when the pane is mounted again. Without a source the tab says the wiki cannot be read.
 * Its keys come from the key router (keys/wiring.ts), which calls handleKey and command: the view listens for
 * none itself (standaloneKeys: false), so no key runs twice.
 */
export const mountWiki: MountPane = (root, ctx): PaneView => {
  const { document: doc, copy, intents } = ctx
  const source = ctx.wiki
  if (!source) {
    root.replaceChildren(emptyState(doc, { icon: "book-open", title: copy.shell.wikiUnavailable }))
    return { render() {} }
  }
  let index: IssueIndex | null = null
  let started = false
  /** The last listing failed: the tab reads it again when it shows next. */
  let failed = false
  let retry = false
  const view = createWikiView(doc, {
    words: copy.wiki,
    loadPage: (slug) => source.page(slug),
    searchPages: source.search ? (query) => source.search!(query) : undefined,
    standaloneKeys: false,
    // An issue id in a page: the list tab, at the place the row is listed, with the row chosen.
    onIssue: (id) => {
      intents.dispatch({ type: "tab", tab: "list" })
      const at = index?.find(id)
      if (at !== undefined) intents.enter(index!.homeOf(at), id)
      intents.choose(id)
    },
  })
  root.replaceChildren(view.el)

  async function list(): Promise<void> {
    failed = false
    view.setLoading(true)
    try {
      const parsed = parseWikiLs(await source!.list(), copy.wiki)
      if (parsed.ok) view.setPages(parsed.value.pages, parsed.value.dir)
      else view.setError(fill(copy.shell.wikiListFailed, { reason: parsed.error }))
    } catch (reason) {
      failed = true
      view.setError(fill(copy.shell.wikiListFailed, { reason: reason instanceof Error ? reason.message : String(reason) }))
    }
  }

  return {
    render(state, current) {
      index = current
      view.setWidth(ctx.layout().width)
      if (started) {
        if (retry) {
          retry = false
          void list()
        }
        return
      }
      started = true
      // The saved page first: the listing opens the first page only when no page is shown.
      void view.restore(state.wiki).then(list)
    },
    focus: () => view.focusPane(view.state().pane),
    handleKey: (event) => view.handleKey(event),
    command: (action) => view.command(action),
    toggleRaw: () => view.toggleRaw(),
    raw: () => view.state().raw,
    leave: () => {
      retry = failed
      intents.dispatch({ type: "wiki", wiki: view.state() })
    },
    destroy: () => view.destroy(),
  }
}
