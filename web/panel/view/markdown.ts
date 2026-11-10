import { el } from './components.ts'
import MarkdownIt from 'markdown-it'
import { footnote } from '@mdit/plugin-footnote'
import { cjkFriendly } from '@mdit/plugin-cjk-friendly'
import { katex as katexPlugin } from '@mdit/plugin-katex'
import { snippet } from '@mdit/plugin-snippet'
import katex from 'katex'
import type { Token } from 'markdown-it'
export interface Heading { level: 1 | 2 | 3 | 4 | 5 | 6; text: string; anchor: string }

/** What a link's href means. The renderer never navigates; it records the target on the <a>. */
export type LinkTarget =
  | { kind: "wiki"; slug: string; anchor?: string }
  | { kind: "anchor"; anchor: string }
  | { kind: "external"; href: string }

export interface RenderOptions {
  resolveImage?: (src: string) => string | null
  resolveLink?: (href: string) => LinkTarget
  issueIds?: Iterable<string>
  snippets?: (ref: SnippetRef) => SnippetState
  page?: string
  snippetLoading?: string
}

export interface Rendered {
  root: HTMLDivElement
  headings: Heading[]
  links: HTMLAnchorElement[]
}

/**
 * GitHub slug rule (github-slugger): trim, lowercase; keep letters (any script), combining marks, decimal digits,
 * '-', '_' and spaces; each space -> '-'; drop the rest (punctuation, symbols, emoji, '²', '①'). With `seen`, a
 * repeat gets '-1', '-2', ... and a numbered slug that is already taken is counted again, so `Foo`, `Foo-1`,
 * `Foo` give `foo`, `foo-1`, `foo-2` (moai `wiki::Anchors`).
 *
 * A rule kept on this side: moai's own is src/wiki.rs `Anchors`, and its JSON carries no anchors to read — the page
 * needs them to draw and to jump. Change one, change both.
 */
export function githubAnchor(text: string, seen?: Map<string, number>): string {
  const base = text
    .trim()
    .toLowerCase()
    .replace(/[^\p{Alphabetic}\p{M}\p{Nd}\p{Pc}\- ]/gu, "")
    .replace(/ /g, "-")
  if (!seen) return base
  let slug = base
  while (seen.has(slug)) {
    const count = (seen.get(base) ?? 0) + 1
    seen.set(base, count)
    slug = `${base}-${count}`
  }
  seen.set(slug, 0)
  return slug
}

export interface SnippetRef { path: string; region?: string }
export type SnippetState = { text: string } | { error: string }

function defaultResolve(href: string): LinkTarget {
  if (href.startsWith('#')) {
    let raw = href.slice(1)
    try { raw = decodeURIComponent(raw) } catch { /* retain input */ }
    return { kind: 'anchor', anchor: githubAnchor(raw) }
  }
  return { kind: 'external', href }
}
interface Ctx {
  doc: Document
  resolve: (href: string) => LinkTarget
  issueRe: RegExp | null
  issueSet: Set<string>
}
const at = (items: ArrayLike<string | undefined>, index: number): string => items[index] ?? ''
function makeLink(ctx: Ctx, href: string): HTMLAnchorElement {
  let target: LinkTarget
  try { target = ctx.resolve(href) } catch { target = { kind: "external", href } }
  const a = el(ctx.doc, "a", { class: "m-md-link", tabindex: "-1" })
  if (target && target.kind === "wiki" && typeof target.slug === "string") {
    a.setAttribute("data-kind", "wiki")
    a.setAttribute("data-slug", target.slug)
    if (target.anchor) a.setAttribute("data-anchor", target.anchor)
  } else if (target && target.kind === "anchor" && typeof target.anchor === "string") {
    a.setAttribute("data-kind", "anchor")
    a.setAttribute("data-anchor", target.anchor)
  } else {
    const shown = target && target.kind === "external" && typeof target.href === "string" ? target.href : href
    a.setAttribute("data-kind", "external")
    a.setAttribute("data-href", shown)
    a.setAttribute("title", shown)
  }
  return a
}

function addText(ctx: Ctx, parent: Node, text: string, noLinks: boolean): void {
  const { doc, issueRe } = ctx
  if (noLinks || !issueRe) { parent.appendChild(doc.createTextNode(text)); return }
  issueRe.lastIndex = 0
  let last = 0
  for (const m of text.matchAll(issueRe)) {
    const pos = m.index ?? 0
    if (pos > last) parent.appendChild(doc.createTextNode(text.slice(last, pos)))
    parent.appendChild(el(doc, "a", { class: "m-md-link m-md-issue", tabindex: "-1", "data-kind": "issue", "data-issue": at(m, 0) }, at(m, 0)))
    last = pos + at(m, 0).length
  }
  if (last < text.length) parent.appendChild(doc.createTextNode(text.slice(last)))
}

let parser: InstanceType<typeof MarkdownIt> | undefined
function getParser(): InstanceType<typeof MarkdownIt> {
  if (!globalThis.moaiVendor) throw new Error('Markdown vendor bundle is unavailable')
  if (!parser) {
    parser = new MarkdownIt({ html: false, linkify: false, typographer: false, breaks: false })
    parser.use(footnote).use(cjkFriendly).use(katexPlugin, { delimiters: 'dollars', mathFence: true, output: 'mathml' })
    parser.use(snippet, { currentPath: env => (env as { page?: string }).page ?? '/__body__.md', allowOutsideRoot: true, denyFiles: [] })
    parser.validateLink = () => true
    parser.normalizeLink = u => u
    parser.normalizeLinkText = u => u
  }
  return parser
}
function parse(source: string, page?: string): Token[] {
  const clean = String(source ?? '').replace(/^\uFEFF/, '')
  const lines = clean.split(/\r\n?|\n/)
  const tokens = getParser().parse(clean, { page })
  for (const token of tokens) {
    const meta = token.meta as { requestedPath?: string; region?: string; invalidSnippet?: boolean } | null
    if (!meta?.requestedPath || !token.map) continue
    const line = lines[token.map[0]] ?? ''
    const marker = line.indexOf('<<<')
    if (marker < 0) continue
    const written = line.slice(marker + 3).trim()
    const hash = written.indexOf('#'), brace = written.indexOf('{')
    if (hash < 0 || (brace >= 0 && brace < hash)) { meta.region = ''; continue }
    const region = written.slice(hash + 1, brace < 0 ? undefined : brace)
    // Rust's is_region_name accepts Unicode alphabetic and numeric characters, '_' and '-'.
    if (!/^[\p{Alphabetic}\p{Number}_-]+$/u.test(region)) meta.invalidSnippet = true
    else meta.region = region
  }
  return tokens
}
function refOf(token: Token): SnippetRef | undefined {
  const meta = token.meta as { requestedPath?: string, region?: string, invalidSnippet?: boolean } | null
  return meta?.requestedPath && !meta.invalidSnippet ? { path: meta.requestedPath, ...(meta.region ? { region: meta.region } : {}) } : undefined
}
export function snippetRefs(source: string, env?: { page?: string }): SnippetRef[] {
  const refs: SnippetRef[] = [], seen = new Set<string>()
  for (const t of parse(source, env?.page)) {
    const ref = refOf(t)
    if (!ref) continue
    const key = JSON.stringify(ref)
    if (!seen.has(key) && refs.length < 32) { refs.push(ref); seen.add(key) }
  }
  return refs
}
function anchorText(tokens: Token[]): string {
  return tokens.map(t => t.type === 'text' || t.type === 'code_inline' ? t.content : t.type === 'math_inline' ? `$${t.content}$` : t.type === 'softbreak' || t.type === 'hardbreak' ? '\n' : '').join('')
}
export function renderMarkdown(doc: Document, source: string, options: RenderOptions = {}): Rendered {
  const tokens = parse(source, options.page)
  const sourceLines = String(source ?? '').replace(/^\uFEFF/, '').split(/\r\n?|\n/)
  const ids = [...new Set([...(options.issueIds ?? [])].filter(id => typeof id === 'string' && id.length > 0))].sort((a, b) => b.length - a.length)
  const issueRe = ids.length ? new RegExp(`(?<![\\p{L}\\p{N}_-])(?:${ids.map(id => id.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')).join('|')})(?![\\p{L}\\p{N}_]|[.-][\\p{L}\\p{N}_])`, 'gu') : null
  const ctx: Ctx = { doc, resolve: options.resolveLink ?? defaultResolve, issueRe, issueSet: new Set(ids) }
  const root = el(doc, 'div', { class: 'm-md' }), headings: Heading[] = [], seen = new Map<string, number>()
  const stack: Node[] = [root]
  let insideLink = 0, footnotes = false, snippetCount = 0
  const hrefs: string[] = []
  const parent = () => stack[stack.length - 1]!
  const push = (element: Element) => { parent().appendChild(element); stack.push(element) }
  const pop = () => { if (stack.length > 1) stack.pop() }
  const math = (t: Token, block: boolean) => {
    const host = el(doc, block ? 'div' : 'span', { class: block ? 'm-md-math m-md-math-block' : 'm-md-math' })
    try {
      katex.render(t.content, host, { output: 'mathml', displayMode: block, throwOnError: true, trust: false, strict: 'ignore', maxExpand: 1000, macros: { '\\fcolorbox': '\\colorbox{#2}{#3}' } })
      for (const node of host.querySelectorAll('[style], [href]')) { node.removeAttribute('style'); node.removeAttribute('href') }
    } catch (error) { host.replaceChildren(el(doc, 'code', { class: 'm-md-math-error', title: String(error) }, `$${t.content}$`)) }
    parent().appendChild(host)
  }
  const inline = (children: Token[]) => {
    for (const t of children) {
      switch (t.type) {
        case 'text': addText(ctx, parent(), t.content, insideLink > 0); break
        case 'softbreak': parent().appendChild(doc.createTextNode('\n')); break
        case 'hardbreak': parent().appendChild(el(doc, 'br')); break
        case 'code_inline': {
          const code = el(doc, 'code', { class: 'm-md-inline-code' }, t.content)
          if (!insideLink && ctx.issueSet.has(t.content)) { const a = el(doc, 'a', { class: 'm-md-link m-md-issue', tabindex: '-1', 'data-kind': 'issue', 'data-issue': t.content }); a.appendChild(code); parent().appendChild(a) } else parent().appendChild(code)
          break
        }
        case 'math_inline': math(t, false); break
        case 'link_open': { const href = String(t.attrGet('href') ?? ''); hrefs.push(href); push(makeLink(ctx, href)); insideLink++; break }
        // A link with no text (`[](url)`) shows where it goes rather than standing as an empty, invisible anchor.
        case 'link_close': { const href = hrefs.pop() ?? ''; if (!parent().hasChildNodes()) parent().appendChild(doc.createTextNode(href)); pop(); insideLink--; break }
        case 'image': {
          const src = String(t.attrGet('src') ?? ''), alt = t.content, resolved = options.resolveImage?.(src)
          if (resolved) parent().appendChild(el(doc, 'img', { class: 'm-md-image', src: resolved, alt, loading: 'lazy' }))
          else parent().appendChild(doc.createTextNode(`![${alt}](${src})`))
          break
        }
        case 'footnote_ref': {
          const meta = t.meta as { id: number, subId: number }, n = meta.id + 1, sub = meta.subId ? `:${meta.subId}` : ''
          const sup = el(doc, 'sup', { class: 'm-md-fnref', 'data-anchor': `fnref:${n}${sub}` })
          const a = el(doc, 'a', { class: 'm-md-link', tabindex: '-1', 'data-kind': 'anchor', 'data-anchor': `fn:${n}` }, `[${n}]`)
          sup.appendChild(a); parent().appendChild(sup); break
        }
        default:
          if (t.nesting === 1) push(el(doc, t.tag === 's' ? 'del' : t.tag as 'span'))
          else if (t.nesting === -1) pop()
          else if (t.content) addText(ctx, parent(), t.content, insideLink > 0)
      }
    }
  }
  for (let i = 0; i < tokens.length; i++) {
    const t = tokens[i]!
    switch (t.type) {
      case 'inline': {
        const children = t.children ?? []
        // Only the start of an item's first paragraph is a task marker: the item itself (tight) or its first <p>
        // (loose), still empty. A later paragraph, or a heading in the item, that opens with `[x]` stays text.
        const host = parent()
        const item = host.nodeName === 'LI' ? host as HTMLElement : host.nodeName === 'P' && host.parentNode?.nodeName === 'LI' && host.parentNode.firstChild === host ? host.parentNode as HTMLElement : null
        if (item && !host.hasChildNodes() && children[0]?.type === 'text') {
          const match = /^\[([ xX])\][ \t]+(?=\S)/.exec(children[0].content)
          if (match) { const checked = match[1] !== ' '; item.classList.add('m-md-task-item'); host.appendChild(el(doc, 'span', { class: 'm-md-task', role: 'checkbox', 'data-checked': String(checked), 'aria-checked': String(checked), 'aria-disabled': 'true' })); host.appendChild(doc.createTextNode(' ')); children[0].content = children[0].content.slice(match[0].length) }
        }
        inline(children); break
      }
      case 'heading_open': {
        const text = anchorText(tokens[i + 1]?.children ?? []), level = Number(t.tag.slice(1)) as Heading['level']
        if (footnotes) { push(el(doc, 'p')); push(el(doc, 'strong')); break }
        const anchor = githubAnchor(text, seen); headings.push({ level, text, anchor }); push(el(doc, t.tag as 'h1', { id: `m-md-${anchor}`, 'data-anchor': anchor })); break
      }
      case 'heading_close': if (footnotes) pop(); pop(); break
      case 'paragraph_open': if (!t.hidden) push(el(doc, 'p')); break
      case 'paragraph_close': if (!t.hidden) pop(); break
      case 'fence': case 'code_block': {
        const ref = refOf(t), lang = t.info.trim().split(/[\s{]/)[0] ?? ''
        if ((t.meta as { invalidSnippet?: boolean } | null)?.invalidSnippet) {
          const line = sourceLines[t.map?.[0] ?? -1] ?? ''
          const marker = line.indexOf('<<<')
          parent().appendChild(el(doc, 'p', {}, marker < 0 ? line : line.slice(marker))); break
        }
        if (ref && ++snippetCount > 32) { parent().appendChild(el(doc, 'p', {}, sourceLines[t.map?.[0] ?? -1] ?? `<<< ${ref.path}`)); break }
        if (ref) {
          const state = options.snippets?.(ref)
          if (state && 'text' in state) parent().appendChild(el(doc, 'pre', { class: 'm-md-code m-md-snippet', 'data-lang': lang, 'data-path': ref.path }, state.text))
          else parent().appendChild(el(doc, 'p', { class: 'm-md-snippet-missing' }, `${ref.path}: ${state && 'error' in state ? state.error : options.snippetLoading ?? 'Loading'}`))
        } else {
          const pre = el(doc, 'pre', { class: 'm-md-code', ...(lang ? { 'data-lang': lang } : {}) }, lang === 'mermaid' ? t.content : t.content.replace(/\n$/, ''))
          if (lang === 'mermaid') { const figure = el(doc, 'figure', { class: 'm-md-mermaid' }); figure.appendChild(pre); parent().appendChild(figure) } else parent().appendChild(pre)
        }
        break
      }
      case 'math_block': math(t, true); break
      case 'hr': parent().appendChild(el(doc, 'hr')); break
      case 'table_open': push(el(doc, 'div', { class: 'm-md-tablewrap' })); push(el(doc, 'table')); break
      case 'table_close': pop(); pop(); break
      case 'footnote_block_open': footnotes = true; push(el(doc, 'section', { class: 'm-md-footnotes' })); parent().appendChild(el(doc, 'hr')); push(el(doc, 'ol')); break
      case 'footnote_block_close': pop(); pop(); footnotes = false; break
      case 'footnote_open': push(el(doc, 'li', { 'data-anchor': `fn:${(t.meta as { id: number }).id + 1}` })); break
      case 'footnote_close': pop(); break
      case 'footnote_anchor': parent().appendChild(el(doc, 'a', { class: 'm-md-link', tabindex: '-1', 'data-kind': 'anchor', 'data-anchor': `fnref:${(t.meta as { id: number }).id + 1}${(t.meta as { subId: number }).subId ? `:${(t.meta as { subId: number }).subId}` : ''}` }, '↩')); break
      default:
        if (t.nesting === 1) {
          const node = el(doc, t.tag as 'div')
          // markdown-it gives `start` only when it is not 1 — and it may be 0, so test for presence, not truth.
          const start = t.type === 'ordered_list_open' ? t.attrGet('start') : null
          if (start !== null && start !== undefined) node.setAttribute('start', String(start))
          const align = /^text-align:(left|center|right)$/.exec(String(t.attrGet('style') ?? ''))?.[1]
          if (align) node.setAttribute('data-align', align)
          push(node)
        } else if (t.nesting === -1) pop()
        else if (t.content) parent().appendChild(doc.createTextNode(t.content))
    }
  }
  return { root, headings, links: [...root.querySelectorAll<HTMLAnchorElement>('a.m-md-link')] }
}
