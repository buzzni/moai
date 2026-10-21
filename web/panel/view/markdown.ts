// A small, safe markdown renderer for the wiki tab. Block pass first, then an inline pass; the DOM is built
// with createElement and text nodes only, so nothing the page says can become markup. Raw HTML stays literal
// text, images are never created, and links carry no href: the target lives in data-* attributes and the view
// decides what a click does.

import { el } from "./components.ts"

export interface Heading { level: 1 | 2 | 3 | 4 | 5 | 6; text: string; anchor: string }

/** What a link's href means. The renderer never navigates; it records the target on the <a>. */
export type LinkTarget =
  | { kind: "wiki"; slug: string; anchor?: string }
  | { kind: "anchor"; anchor: string }
  | { kind: "external"; href: string }

export interface RenderOptions {
  resolveLink?: (href: string) => LinkTarget
  issueIds?: Iterable<string>
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

function defaultResolve(href: string): LinkTarget {
  if (href.startsWith("#")) {
    let raw = href.slice(1)
    try { raw = decodeURIComponent(raw) } catch { /* keep as written */ }
    return { kind: "anchor", anchor: githubAnchor(raw) }
  }
  return { kind: "external", href }
}

interface Ctx {
  doc: Document
  resolve: (href: string) => LinkTarget
  issueRe: RegExp | null
  issueSet: Set<string>
  headings: Heading[]
  seen: Map<string, number>
  refs: Map<string, string>
}

export function renderMarkdown(doc: Document, source: string, options: RenderOptions = {}): Rendered {
  const ids = [...new Set([...(options.issueIds ?? [])].filter((id) => typeof id === "string" && id.length > 0))]
  ids.sort((a, b) => b.length - a.length)
  const issueRe = ids.length
    ? new RegExp(
        `(?<![\\p{L}\\p{N}_-])(?:${ids.map((id) => id.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("|")})(?![\\p{L}\\p{N}_]|[.-][\\p{L}\\p{N}_])`,
        "gu",
      )
    : null
  const ctx: Ctx = {
    doc,
    resolve: options.resolveLink ?? defaultResolve,
    issueRe,
    issueSet: new Set(ids),
    headings: [],
    seen: new Map(),
    refs: new Map(),
  }
  const root = el(doc, "div", { class: "m-md" })
  // A leading BOM (Windows editors) would keep the first `# Title` from reading as a heading; moai strips it the
  // same way before parsing (`wiki::unmarked`), while `show --json` hands the body over with it.
  const lines = String(source ?? "")
    .replace(/^﻿/, "")
    .replace(/\r\n?/g, "\n")
    .split("\n")
    .map((line) => line.replace(/^[ \t]*\t[ \t]*/, (lead) => lead.replace(/\t/g, "    ")))
  parseBlocks(ctx, collectDefinitions(ctx, lines), root, false)
  const links = [...root.querySelectorAll<HTMLAnchorElement>("a.m-md-link")]
  return { root, headings: ctx.headings, links }
}

// ---- reference definitions ----

const REF_DEF = /^ {0,3}\[((?:[^\[\]\\]|\\.)+)\]:[ \t]*(?:<([^<>\n]*)>|([^\s<][^\s]*))(?:[ \t]+(?:"[^"]*"|'[^']*'|\([^()]*\)))?[ \t]*$/

const refLabel = (label: string): string => label.trim().replace(/\s+/g, " ").toLowerCase()

/** Pull `[label]: url "title"` lines out of the document (outside fenced code); the first definition of a label wins. */
function collectDefinitions(ctx: Ctx, lines: string[]): string[] {
  const out: string[] = []
  let fence: RegExp | null = null
  for (const line of lines) {
    if (fence) {
      if (fence.test(line)) fence = null
      out.push(line)
      continue
    }
    const open = FENCE.exec(line)
    if (open && isFence(line)) {
      const mark = at(open, 2)
      fence = new RegExp(`^ {0,3}${mark[0] === "`" ? "`" : "~"}{${mark.length},}[ \\t]*$`)
      out.push(line)
      continue
    }
    const def = REF_DEF.exec(line)
    const label = def ? refLabel(at(def, 1)) : ""
    const dest = def ? (def[2] ?? def[3] ?? "") : ""
    if (def && label !== "" && dest !== "") {
      if (!ctx.refs.has(label)) ctx.refs.set(label, decodeEntities(dest.replace(/\\([!-/:-@[-`{-~])/g, "$1")))
      continue
    }
    out.push(line)
  }
  return out
}

// ---- block pass ----

const at = (items: ArrayLike<string | undefined>, index: number): string => items[index] ?? ""

const FENCE = /^( {0,3})(`{3,}|~{3,})(.*)$/
const ATX = /^ {0,3}(#{1,6})(?:[ \t]+(.*?))?(?:[ \t]+#+)?[ \t]*$/
const SETEXT = /^ {0,3}(=+|-+)[ \t]*$/
const HR =/^ {0,3}([-*_])(?:[ \t]*\1){2,}[ \t]*$/
const QUOTE = /^ {0,3}>/
const LIST = /^( *)([-+*]|\d{1,9}[.)])( +|$)/
const DELIM_CELL = /^:?-+:?$/

const isBlank = (line: string): boolean => line.trim() === ""
const indentOf = (line: string): number => line.length - line.trimStart().length

function isFence(line: string): boolean {
  const m = FENCE.exec(line)
  return !!m && !(at(m, 2).charAt(0) === "`" && at(m, 3).includes("`"))
}

/** A line that ends a paragraph. Only non-empty bullets and ordered lists starting at 1 may interrupt. */
function interrupts(line: string): boolean {
  if (isFence(line) || ATX.test(line) || HR.test(line) || QUOTE.test(line)) return true
  const m = LIST.exec(line)
  if (m && at(m, 1).length < 4 && at(m, 3) !== "") {
    return !/\d/.test(at(m, 2).charAt(0)) || parseInt(at(m, 2), 10) === 1
  }
  return false
}

function splitRow(line: string): string[] {
  let s = line.trim()
  if (s.startsWith("|")) s = s.slice(1)
  if (s.endsWith("|") && !s.endsWith("\\|")) s = s.slice(0, -1)
  const cells: string[] = []
  let cell = ""
  for (let i = 0; i < s.length; i++) {
    if (s.charAt(i) === "\\" && s.charAt(i + 1) === "|") { cell += "|"; i++ }
    else if (s.charAt(i) === "|") { cells.push(cell.trim()); cell = "" }
    else cell += s.charAt(i)
  }
  cells.push(cell.trim())
  return cells
}

function tableAt(lines: string[], i: number): { head: string[]; aligns: string[] } | null {
  const next = at(lines, i + 1)
  if (next === undefined || !at(lines, i).includes("|") || !next.includes("-")) return null
  const delim = splitRow(next)
  if (!delim.every((cell) => DELIM_CELL.test(cell))) return null
  const head = splitRow(at(lines, i))
  if (head.length !== delim.length) return null
  const aligns = delim.map((cell) => (cell.startsWith(":") ? (cell.endsWith(":") ? "center" : "left") : cell.endsWith(":") ? "right" : ""))
  return { head, aligns }
}

/** Append a heading: inline content, then its GitHub anchor (numbered on repeats) as id and data-anchor. */
function appendHeading(ctx: Ctx, parent: Node, level: Heading["level"], source: string): void {
  const h = el(ctx.doc, `h${level}` as "h1")
  inlineInto(ctx, h, source.trim(), false)
  const text = h.textContent ?? ""
  const anchor = githubAnchor(text, ctx.seen)
  h.setAttribute("id", `m-md-${anchor}`)
  h.setAttribute("data-anchor", anchor)
  ctx.headings.push({ level, text, anchor })
  parent.appendChild(h)
}

const BLOCK_TAGS = new Set(["P", "PRE", "H1", "H2", "H3", "H4", "H5", "H6", "HR", "BLOCKQUOTE", "UL", "OL", "DIV"])

/** True when `node` ends in inline content (a bare paragraph of a tight list item) rather than a block. */
function endsInline(node: Node): boolean {
  const last = node.lastChild
  return last !== null && !(last.nodeType === 1 && BLOCK_TAGS.has((last as Element).tagName.toUpperCase()))
}

function parseBlocks(ctx: Ctx, lines: string[], parent: Node, tight: boolean): void {
  const { doc } = ctx
  const n = lines.length
  let i = 0
  while (i < n) {
    const line = at(lines, i)
    if (isBlank(line)) { i++; continue }

    // Fenced code.
    const fence = FENCE.exec(line)
    if (fence && isFence(line)) {
      const indent = at(fence, 1).length
      const mark = at(fence, 2)
      const close = new RegExp(`^ {0,3}${mark[0] === "`" ? "`" : "~"}{${mark.length},}[ \\t]*$`)
      const body: string[] = []
      i++
      while (i < n && !close.test(at(lines, i))) {
        body.push(at(lines, i).replace(new RegExp(`^ {0,${indent}}`), ""))
        i++
      }
      i++
      const info = at(fence, 3).trim().split(/\s+/)[0] ?? ""
      const code = el(doc, "code", {}, body.join("\n"))
      parent.appendChild(el(doc, "pre", { class: "m-md-code", "data-lang": info || null }, code))
      continue
    }

    // Indented code.
    if (/^ {4}/.test(line)) {
      const body: string[] = []
      while (i < n && (/^ {4}/.test(at(lines, i)) || (isBlank(at(lines, i)) && i + 1 < n && (/^ {4}/.test(at(lines, i + 1)) || isBlank(at(lines, i + 1)))))) {
        body.push(at(lines, i).slice(4))
        i++
      }
      while (body.length && isBlank(at(body, body.length - 1))) body.pop()
      parent.appendChild(el(doc, "pre", { class: "m-md-code" }, el(doc, "code", {}, body.join("\n"))))
      continue
    }

    // Heading.
    const atx = ATX.exec(line)
    if (atx) {
      appendHeading(ctx, parent, at(atx, 1).length as Heading["level"], at(atx, 2))
      i++
      continue
    }

    if (HR.test(line)) { parent.appendChild(el(doc, "hr")); i++; continue }

    // Block quote.
    if (QUOTE.test(line)) {
      const inner: string[] = []
      // A fence opened inside the quote ends where the quote ends: a line without `>` is not lazy text then.
      let quoteFence: RegExp | null = null
      while (i < n && !isBlank(at(lines, i))) {
        if (QUOTE.test(at(lines, i))) {
          const text = at(lines, i).replace(/^ {0,3}> ?/, "")
          const open = FENCE.exec(text)
          if (quoteFence) { if (quoteFence.test(text)) quoteFence = null }
          else if (open && isFence(text)) {
            const mark = at(open, 2)
            quoteFence = new RegExp(`^ {0,3}${mark[0] === "`" ? "`" : "~"}{${mark.length},}[ \\t]*$`)
          }
          inner.push(text)
        } else if (quoteFence || interrupts(at(lines, i))) break
        else inner.push(at(lines, i))
        i++
      }
      const quote = el(doc, "blockquote")
      parseBlocks(ctx, inner, quote, false)
      parent.appendChild(quote)
      continue
    }

    // List.
    const first = LIST.exec(line)
    if (first && at(first, 1).length < 4) {
      i = parseList(ctx, lines, i, parent)
      continue
    }

    // Table.
    const table = tableAt(lines, i)
    if (table) {
      const cols = table.head.length
      const wrap = el(doc, "div", { class: "m-md-tablewrap" })
      const tbl = el(doc, "table")
      const headRow = el(doc, "tr")
      table.head.forEach((cell, c) => {
        const th = el(doc, "th", { "data-align": at(table.aligns, c) || null })
        inlineInto(ctx, th, cell, false)
        headRow.appendChild(th)
      })
      tbl.appendChild(el(doc, "thead", {}, headRow))
      const tbody = el(doc, "tbody")
      i += 2
      while (i < n && !isBlank(at(lines, i)) && !interrupts(at(lines, i))) {
        const cells = splitRow(at(lines, i))
        const tr = el(doc, "tr")
        for (let c = 0; c < cols; c++) {
          const td = el(doc, "td", { "data-align": at(table.aligns, c) || null })
          inlineInto(ctx, td, at(cells, c) ?? "", false)
          tr.appendChild(td)
        }
        tbody.appendChild(tr)
        i++
      }
      tbl.appendChild(tbody)
      wrap.appendChild(tbl)
      parent.appendChild(wrap)
      continue
    }

    // Paragraph, or a setext heading when a `===` / `---` line underlines it.
    const para: string[] = [line.trimStart()]
    let setext: Heading["level"] | 0 = 0
    i++
    while (i < n && !isBlank(at(lines, i))) {
      const underline = SETEXT.exec(at(lines, i))
      if (underline) {
        setext = at(underline, 1).charAt(0) === "=" ? 1 : 2
        i++
        break
      }
      if (interrupts(at(lines, i)) || tableAt(lines, i)) break
      para.push(at(lines, i).trimStart())
      i++
    }
    const text = para.join("\n").replace(/\s+$/, "")
    if (setext) appendHeading(ctx, parent, setext, text)
    // A tight item's paragraph is bare text — unless bare text already ends the item (a blank line before a
    // nested list keeps the list tight), where it gets a <p> so the two paragraphs do not run together.
    else if (tight && !endsInline(parent)) inlineInto(ctx, parent, text, false)
    else {
      const p = el(doc, "p")
      inlineInto(ctx, p, text, false)
      parent.appendChild(p)
    }
  }
}

function parseList(ctx: Ctx, lines: string[], start: number, parent: Node): number {
  const { doc } = ctx
  const n = lines.length
  const head = LIST.exec(at(lines, start)) as RegExpExecArray
  const ordered = /\d/.test(at(head, 2).charAt(0))
  const delim = at(head, 2).charAt(at(head, 2).length - 1)
  const sameKind = (line: string): RegExpExecArray | null => {
    const m = LIST.exec(line)
    if (!m || at(m, 1).length >= 4 || HR.test(line)) return null
    if (/\d/.test(at(m, 2).charAt(0)) !== ordered || at(m, 2).charAt(at(m, 2).length - 1) !== delim) return null
    return m
  }
  const items: string[][] = []
  let loose = false
  let i = start
  while (i < n) {
    const m = sameKind(at(lines, i))
    if (!m) break
    const gap = at(m, 3).length >= 1 && at(m, 3).length <= 4 ? at(m, 3).length : 1
    const contentIndent = at(m, 1).length + at(m, 2).length + gap
    const itemLines = [at(m, 3) === "" ? "" : at(lines, i).slice(contentIndent)]
    i++
    while (i < n) {
      const l = at(lines, i)
      if (isBlank(l)) {
        let j = i
        while (j < n && isBlank(at(lines, j))) j++
        if (j < n && indentOf(at(lines, j)) >= contentIndent) {
          for (; i < j; i++) itemLines.push("")
          continue
        }
        break
      }
      if (indentOf(l) >= contentIndent) { itemLines.push(l.slice(contentIndent)); i++; continue }
      if (!isBlank(at(itemLines, itemLines.length - 1)) && !interrupts(l) && !LIST.test(l) && !tableAt(lines, i)) {
        itemLines.push(l.trimStart())
        i++
        continue
      }
      break
    }
    // A blank line between two blocks of one item makes the list loose, unless a nested list follows it.
    const blankBetween = itemLines.some((l, k) => k > 0 && isBlank(l) && !isBlank(at(itemLines, k - 1)) && itemLines.slice(k).some((x) => !isBlank(x)))
    if (blankBetween && !itemLines.some((l) => LIST.test(l))) loose = true
    while (itemLines.length && isBlank(at(itemLines, itemLines.length - 1))) itemLines.pop()
    items.push(itemLines)
    let j = i
    while (j < n && isBlank(at(lines, j))) j++
    if (j < n && sameKind(at(lines, j))) {
      if (j > i) loose = true
      i = j
      continue
    }
    break
  }
  const list = el(doc, ordered ? "ol" : "ul")
  const firstNum = ordered ? parseInt(at(head, 2), 10) : 1
  if (ordered && firstNum !== 1) list.setAttribute("start", String(firstNum))
  for (const itemLines of items) {
    const li = el(doc, "li")
    const task = /^\[([ xX])\][ \t]+(?=\S)/.exec(at(itemLines, 0))
    if (task) itemLines[0] = at(itemLines, 0).slice(at(task, 0).length)
    parseBlocks(ctx, itemLines, li, !loose)
    if (task) {
      // Only the start of the first paragraph; a span, never an <input>.
      const checked = at(task, 1) !== " "
      const mark = el(doc, "span", {
        class: "m-md-task",
        role: "checkbox",
        "data-checked": checked ? "true" : "false",
        "aria-checked": checked ? "true" : "false",
        "aria-disabled": "true",
      })
      const first = li.firstChild
      const host: Node = first && first.nodeType === 1 && (first as Element).tagName === "P" ? first : li
      host.insertBefore(mark, host.firstChild)
      host.insertBefore(doc.createTextNode(" "), mark.nextSibling)
      li.setAttribute("class", "m-md-task-item")
    }
    list.appendChild(li)
  }
  parent.appendChild(list)
  return i
}

// ---- inline pass ----

const PUNCT = /[!-/:-@[-`{-~]/
const WORD = /[\p{L}\p{N}]/u
const AUTOLINK = /<([a-zA-Z][a-zA-Z0-9+.-]{1,31}:[^\s<>]*)>/y

const ENTITIES: Record<string, string> = {
  amp: "&", lt: "<", gt: ">", quot: '"', apos: "'", nbsp: "\u00a0", copy: "\u00a9", reg: "\u00ae", trade: "\u2122",
  mdash: "\u2014", ndash: "\u2013", hellip: "\u2026", laquo: "\u00ab", raquo: "\u00bb", middot: "\u00b7",
  bull: "\u2022", rarr: "\u2192", larr: "\u2190", times: "\u00d7",
}
const ENTITY = /&(?:#([0-9]{1,7})|#[xX]([0-9a-fA-F]{1,6})|([a-zA-Z][a-zA-Z0-9]{1,8}));/y

/** The text one entity at `i` stands for, with its length; null leaves the `&` literal. */
function entityAt(s: string, i: number): { text: string; len: number } | null {
  ENTITY.lastIndex = i
  const m = ENTITY.exec(s)
  if (!m) return null
  const len = at(m, 0).length
  if (m[3] !== undefined) {
    const named = Object.hasOwn(ENTITIES, m[3]) ? ENTITIES[m[3]] : undefined
    return named === undefined ? null : { text: named, len }
  }
  const cp = m[1] !== undefined ? parseInt(m[1], 10) : parseInt(m[2] ?? "", 16)
  const control = cp < 0x20 ? cp !== 9 && cp !== 10 : cp >= 0x7f && cp <= 0x9f
  if (cp === 0 || control || (cp >= 0xd800 && cp <= 0xdfff) || cp > 0x10ffff) return null
  return { text: String.fromCodePoint(cp), len }
}

function decodeEntities(s: string): string {
  if (!s.includes("&")) return s
  let out = ""
  for (let i = 0; i < s.length; i++) {
    const e = s.charAt(i) === "&" ? entityAt(s, i) : null
    if (e) { out += e.text; i += e.len - 1 } else out += s.charAt(i)
  }
  return out
}

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

/** The code span opening at i: its backtick run length and the index of the closing run (-1 if none). */
function codeSpanEnd(s: string, i: number): { run: number; end: number } {
  let run = 1
  while (s.charAt(i + run) === "`") run++
  let j = i + run
  while (j < s.length) {
    const k = s.indexOf("`", j)
    if (k < 0) break
    let r = 1
    while (s.charAt(k + r) === "`") r++
    if (r === run) return { run, end: k }
    j = k + r
  }
  return { run, end: -1 }
}

interface LinkMatch { text: string; dest: string; end: number }

/** Index of the `]` closing the `[` at i, or -1. */
function bracketEnd(s: string, i: number): number {
  let depth = 1
  let j = i + 1
  for (; j < s.length; j++) {
    const ch = s.charAt(j)
    if (ch === "\\") { j++; continue }
    if (ch === "`") {
      const span = codeSpanEnd(s, j)
      j = span.end >= 0 ? span.end + span.run - 1 : j + span.run - 1
      continue
    }
    if (ch === "[") depth++
    else if (ch === "]" && --depth === 0) break
  }
  return depth === 0 ? j : -1
}

function parseLinkAt(s: string, i: number): LinkMatch | null {
  const j = bracketEnd(s, i)
  if (j < 0 || s.charAt(j + 1) !== "(") return null
  const text = s.slice(i + 1, j)
  let k = j + 2
  while (s.charAt(k) === " " || s.charAt(k) === "\n") k++
  let dest = ""
  if (s.charAt(k) === "<") {
    const close = s.indexOf(">", k)
    if (close < 0 || s.slice(k, close).includes("\n")) return null
    dest = s.slice(k + 1, close)
    k = close + 1
  } else {
    let parens = 0
    for (; k < s.length; k++) {
      const ch = s.charAt(k)
      if (ch === "\\" && k + 1 < s.length && PUNCT.test(s.charAt(k + 1))) { dest += s.charAt(++k); continue }
      if (/\s/.test(ch)) break
      if (ch === "(") parens++
      else if (ch === ")") { if (parens === 0) break; parens-- }
      dest += ch
    }
  }
  while (s.charAt(k) === " " || s.charAt(k) === "\n") k++
  const quote = s.charAt(k)
  if (quote === '"' || quote === "'" || quote === "(") {
    const closer = quote === "(" ? ")" : quote
    let t = k + 1
    while (t < s.length && s.charAt(t) !== closer) t += s.charAt(t) === "\\" ? 2 : 1
    if (t >= s.length) return null
    k = t + 1
    while (s.charAt(k) === " " || s.charAt(k) === "\n") k++
  }
  if (s.charAt(k) !== ")" || dest === "") return null
  return { text, dest: decodeEntities(dest), end: k + 1 }
}

/** `[text][ref]`, `[ref][]` and `[ref]` against the collected definitions. */
function parseRefAt(ctx: Ctx, s: string, i: number): LinkMatch | null {
  if (ctx.refs.size === 0) return null
  const j = bracketEnd(s, i)
  if (j < 0) return null
  const text = s.slice(i + 1, j)
  if (s.charAt(j + 1) === "[") {
    const k = s.indexOf("]", j + 2)
    const inner = k < 0 ? null : s.slice(j + 2, k)
    if (inner !== null && !inner.includes("[")) {
      const dest = ctx.refs.get(refLabel(inner === "" ? text : inner))
      if (dest !== undefined) return { text, dest, end: k + 1 }
    }
  }
  const dest = ctx.refs.get(refLabel(text))
  return dest === undefined || text.trim() === "" ? null : { text, dest, end: j + 1 }
}

interface EmphMatch { lit: number; k: number; start: number; close: number; end: number }
type EmphMemo = Map<number, EmphMatch | null> & { lastClose?: Record<string, number> }

/**
 * The emphasis opening at i, or null. Memoized per position of `s`: an opener that never closes is retried by
 * every opener before it, which without the memo is exponential (`*a *b *c ...` froze the panel at ~25 openers).
 */
function matchEmph(s: string, i: number, memo: EmphMemo): EmphMatch | null {
  const known = memo.get(i)
  if (known !== undefined) return known
  const found = findEmph(s, i, memo)
  memo.set(i, found)
  return found
}

function findEmph(s: string, i: number, memo: EmphMemo): EmphMatch | null {
  const c = s.charAt(i)
  let r = 1
  while (s.charAt(i + r) === c) r++
  const before = s.charAt(i - 1)
  const after = s.charAt(i + r)
  if (after === undefined || /\s/.test(after)) return null
  if (c === "_" && before !== undefined && WORD.test(before)) return null
  // No run of `c` after this opener can close it (none is preceded by a non-space): nothing to scan for.
  if ((memo.lastClose?.[c] ?? Infinity) < i + r) return null
  const sizes = r >= 3 ? [3, 2, 1] : r === 2 ? [2, 1] : [1]
  for (const k of sizes) {
    let j = i + r
    while (j < s.length) {
      const ch = s.charAt(j)
      if (ch === "\\") { j += 2; continue }
      if (ch === "`") {
        const span = codeSpanEnd(s, j)
        j = span.end >= 0 ? span.end + span.run : j + span.run
        continue
      }
      if (ch !== c) { j++; continue }
      let rr = 1
      while (s.charAt(j + rr) === c) rr++
      const prev = s.charAt(j - 1)
      const next = s.charAt(j + rr)
      const canClose = prev !== undefined && !/\s/.test(prev) && (c !== "_" || next === undefined || !WORD.test(next))
      const canOpen = next !== undefined && !/\s/.test(next) && (c !== "_" || prev === undefined || !WORD.test(prev))
      if (canClose && rr >= k) return { lit: r - k, k, start: i + r, close: j, end: j + k }
      if (canOpen && !canClose) {
        const nested = matchEmph(s, j, memo)
        if (nested) { j = nested.end; continue }
      }
      j += rr
    }
  }
  return null
}

function inlineInto(ctx: Ctx, parent: Node, s: string, noLinks: boolean): void {
  const { doc } = ctx
  // Fill the emphasis memo right to left first: every nested lookup in findEmph is then a memo hit, so the
  // recursion stays one level deep however many openers the text has.
  const emphMemo: EmphMemo = new Map()
  const lastClose: Record<string, number> = { "*": -1, _: -1 }
  for (let p = 1; p < s.length; p++) {
    const ch = s.charAt(p)
    // A closing run starts after a non-space that is not the same delimiter (or after an escaped one).
    const prev = s.charAt(p - 1)
    if ((ch === "*" || ch === "_") && !/\s/.test(prev) && (prev !== ch || s.charAt(p - 2) === "\\")) lastClose[ch] = p
  }
  emphMemo.lastClose = lastClose
  for (let p = s.length - 1; p >= 0; p--) {
    const ch = s.charAt(p)
    if (ch === "*" || ch === "_") matchEmph(s, p, emphMemo)
  }
  let strikeFail = Infinity // an opener at or after this index has no closer
  let buf = ""
  const flush = (): void => {
    if (buf) addText(ctx, parent, buf, noLinks)
    buf = ""
  }
  let i = 0
  while (i < s.length) {
    const c = s.charAt(i)
    if (c === "\\") {
      const nx = s.charAt(i + 1)
      if (nx === "\n") { flush(); parent.appendChild(el(doc, "br")); i += 2; continue }
      if (nx !== undefined && PUNCT.test(nx)) { buf += nx; i += 2; continue }
      buf += c
      i++
      continue
    }
    if (c === "\n") {
      if (/ {2,}$/.test(buf)) { buf = buf.replace(/ +$/, ""); flush(); parent.appendChild(el(doc, "br")) }
      else buf = buf.replace(/ +$/, "") + "\n"
      i++
      continue
    }
    if (c === "`") {
      const span = codeSpanEnd(s, i)
      if (span.end >= 0) {
        let body = s.slice(i + span.run, span.end).replace(/\n/g, " ")
        if (body.length > 2 && body.startsWith(" ") && body.endsWith(" ") && body.trim() !== "") body = body.slice(1, -1)
        flush()
        const code = el(doc, "code", { class: "m-md-inline-code" }, body)
        const id = body.trim()
        if (!noLinks && ctx.issueSet.has(id)) {
          // A code span that is exactly an issue id links like a bare id, and still looks like code.
          parent.appendChild(el(doc, "a", { class: "m-md-link m-md-issue", tabindex: "-1", "data-kind": "issue", "data-issue": id }, code))
        } else parent.appendChild(code)
        i = span.end + span.run
      } else {
        buf += "`".repeat(span.run)
        i += span.run
      }
      continue
    }
    if (c === "!" && s.charAt(i + 1) === "[") {
      const m = noLinks ? null : parseLinkAt(s, i + 1)
      if (m) { flush(); parent.appendChild(doc.createTextNode(s.slice(i, m.end))); i = m.end }
      else { buf += c; i++ }
      continue
    }
    if (c === "[" && !noLinks) {
      const m = parseLinkAt(s, i) ?? parseRefAt(ctx, s, i)
      if (m) {
        flush()
        const a = makeLink(ctx, m.dest)
        inlineInto(ctx, a, m.text, true)
        if (!a.firstChild) a.appendChild(doc.createTextNode(m.dest))
        parent.appendChild(a)
        i = m.end
        continue
      }
    }
    if (c === "<") {
      AUTOLINK.lastIndex = i
      const m = AUTOLINK.exec(s)
      if (m) {
        flush()
        if (noLinks) parent.appendChild(doc.createTextNode(at(m, 0)))
        else {
          const a = makeLink(ctx, at(m, 1))
          a.appendChild(doc.createTextNode(at(m, 1)))
          parent.appendChild(a)
        }
        i += at(m, 0).length
        continue
      }
    }
    if (c === "&") {
      const e = entityAt(s, i)
      if (e) { buf += e.text; i += e.len; continue }
    }
    if (c === "~" && s.charAt(i + 1) === "~" && s.charAt(i + 2) !== "~" && i < strikeFail) {
      const open = s.charAt(i + 2)
      let close = -1
      if (open !== "" && !/\s/.test(open)) {
        for (let k = s.indexOf("~~", i + 2); k >= 0; k = s.indexOf("~~", k + 1)) {
          if (k > i + 2 && s.charAt(k + 2) !== "~" && s.charAt(k - 1) !== "~" && !/\s/.test(s.charAt(k - 1))) { close = k; break }
        }
      }
      if (close >= 0) {
        flush()
        const d = el(doc, "del")
        parent.appendChild(d)
        inlineInto(ctx, d, s.slice(i + 2, close), noLinks)
        i = close + 2
        continue
      }
      strikeFail = i
    }
    if (c === "*" || c === "_") {
      const m = matchEmph(s, i, emphMemo)
      if (m) {
        buf += c.repeat(m.lit)
        flush()
        let host: Node = parent
        const tags = m.k === 3 ? ["strong", "em"] : m.k === 2 ? ["strong"] : ["em"]
        for (const tag of tags) {
          const e = el(doc, tag as "em")
          host.appendChild(e)
          host = e
        }
        inlineInto(ctx, host, s.slice(m.start, m.close), noLinks)
        i = m.end
        continue
      }
      let r = 1
      while (s.charAt(i + r) === c) r++
      buf += c.repeat(r)
      i += r
      continue
    }
    buf += c
    i++
  }
  flush()
}
