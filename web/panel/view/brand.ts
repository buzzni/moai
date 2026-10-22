/**
 * The header above the tab bar, drawn as the TUI draws its own (moai src/tui/draw.rs `header`, se-qkga): the logo's
 * five lines and its tagline, dim, then a line down, then `User` and `Version`, their labels dim and padded to one
 * column. The TUI's labels are English in every language, and so are these. A short panel leaves the header out and a
 * narrow one the logo (shell.css), as the TUI does on a short or narrow screen. What it says comes from `/api/boot`
 * (data/about.ts; upstream read it from `status`, which moai's does not carry).
 *
 * Right of User and Version stands the TUI's third column, the registered projects (draw.rs `with_projects`,
 * moai-o14f): `<1> name` … `<9> name`, six to a column, the tenth and on without a number. The TUI's `<0>` (its
 * all-projects layer) has no page and is left out. The project on screen is marked as the TUI's shimmer marks it — here
 * in the accent, bold, and `aria-current`, so the colour is not alone — and a click opens another. A narrow header
 * drops the logo first and then this column (shell.css).
 */
import { versionLine, type About, type VersionCopy } from '../data/about.ts'
import type { ProjectEntry } from '../data/projects.ts'
import { el } from './components.ts'

/** How long one tip stands before the next (se-irhr). */
export const TIP_MS = 8000

/** The TUI's `LOGO`, letter for letter: the art, then the tagline. */
export const LOGO = [
  '• ▌ ▄ ·.        ▄▄▄· ▪',
  '·██ ▐███▪ ▄█▀▄ ▐█ ▀█ ██',
  '▐█ ▌▐▌▐█·▐█▌.▐▌▄█▀▀█ ▐█·',
  '██ ██▌▐█▌▐█▌.▐▌▐█▪ ▐▌▐█▌',
  '▀▀  █▪▀▀▀ ▀█▄▀▪ ▀  ▀ ▀▀▀',
  'Issue tracker for AI agent',
] as const

/** The label column (`HEADER_LABEL`): `User`, `Version` and the colon after them stand in one column. */
const LABEL = 7
const pad = (text: string, width: number) => text + ' '.repeat(Math.max(0, width - [...text].length))

/** The logo's lines as the TUI lays them: the art pushed as one block to the middle of the tagline's width. */
export function logoLines(): string[] {
  const art = LOGO.slice(0, -1)
  const width = Math.max(...LOGO.map((line) => [...line].length))
  const indent = Math.floor((width - Math.max(...art.map((line) => [...line].length))) / 2)
  return LOGO.map((line, n) => pad(n < art.length ? ' '.repeat(indent) + line : line, width))
}

const SVG = 'http://www.w3.org/2000/svg'

/**
 * The art as a terminal draws it: each block letter fills its cell (one wide, two tall) to the edge, so a row meets the
 * next. A font's glyphs do not (the field test saw a seam under every row, whatever the line height), so the letters
 * become the cells they stand for, in one path: neighbouring blocks are filled together and no hairline shows
 * between them. The dots keep the size they have in a terminal font.
 */
export function artPath(lines: readonly string[]): string {
  const parts: string[] = []
  const rect = (x: number, y: number, w: number, h: number) => parts.push(`M${x} ${y}h${w}v${h}h${-w}z`)
  const dot = (cx: number, cy: number, r: number) => parts.push(`M${cx - r} ${cy}a${r} ${r} 0 1 0 ${2 * r} 0a${r} ${r} 0 1 0 ${-2 * r} 0z`)
  lines.forEach((line, row) => {
    ;[...line].forEach((letter, x) => {
      const y = row * 2
      switch (letter) {
        case '█': return rect(x, y, 1, 2)
        case '▀': return rect(x, y, 1, 1)
        case '▄': return rect(x, y + 1, 1, 1)
        case '▌': return rect(x, y, 0.5, 2)
        case '▐': return rect(x + 0.5, y, 0.5, 2)
        case '▪': return rect(x + 0.25, y + 0.75, 0.5, 0.5)
        case '•': return dot(x + 0.5, y + 1, 0.28)
        case '·': return dot(x + 0.5, y + 1, 0.14)
        case '.': return dot(x + 0.5, y + 1.6, 0.14)
      }
    })
  })
  return parts.join('')
}

/** The letters {@link artPath} draws; the art holds no other but the space. */
export const ART_LETTERS = '█▀▄▌▐▪•·.'

/** The info lines: label and what it says; a line with no label goes on from the one above (the TUI's tail). */
export function infoLines(about: About | null, copy: VersionCopy, now = Date.now()): Array<[string, string]> {
  const version = versionLine(about, copy, now)
  const lines: Array<[string, string]> = [['User', about?.user ?? '—'], ['Version', version.said]]
  if (version.tail) lines.push(['', version.tail])
  return lines
}

export interface Brand {
  el: HTMLElement
  /** Draws the info lines and the projects (none: no column). */
  render(about: About | null, projects?: readonly ProjectEntry[]): void
}

/** The TUI header's rows (`LOGO.len()`): the project column fills six rows, then starts the next column. */
export const PROJECT_ROWS = 6

/** `<n> ` before a numbered project; the width of `<9> ` in spaces before one without a number (draw.rs `numbered`). */
export const projectKey = (number: number | null): string => (number === null ? ' '.repeat('<9> '.length) : `<${number}> `)

/**
 * The tips the header turns through under the Version line, and what a click on one does (opens the tips page).
 * Without them the header is the TUI's alone.
 */
export interface BrandTips {
  tips: readonly string[]
  label: string
  open(): void
}

export function mountBrand(doc: Document, copy: VersionCopy, tips?: BrandTips, open?: (entry: ProjectEntry) => void): Brand {
  const lines = logoLines()
  const art = lines.slice(0, -1)
  const svg = doc.createElementNS(SVG, 'svg')
  svg.setAttribute('class', 'm-brand-art')
  svg.setAttribute('viewBox', `0 0 ${[...lines[0]!].length} ${art.length * 2}`)
  svg.setAttribute('preserveAspectRatio', 'none')
  const path = doc.createElementNS(SVG, 'path')
  path.setAttribute('d', artPath(art))
  svg.append(path)
  const logo = el(doc, 'div', { class: 'm-brand-logo', 'aria-hidden': 'true' }, svg, el(doc, 'div', { class: 'm-brand-tagline' }, lines.at(-1)!))
  const told = el(doc, 'div', { class: 'm-brand-told' })
  const info = el(doc, 'div', { class: 'm-brand-info' }, told)
  if (tips && tips.tips.length) info.append(tipLine(doc, tips))
  const projects = el(doc, 'nav', { class: 'm-brand-projects', 'aria-label': 'Projects', hidden: true })
  const node = el(doc, 'header', { class: 'm-brand', 'aria-label': 'moai' }, logo, info, projects)
  let drawn = ''
  let drawnProjects = ''
  return {
    el: node,
    render(about, entries = []) {
      const lines = infoLines(about, copy)
      const key = JSON.stringify(lines)
      if (key !== drawn) {
        drawn = key
        told.replaceChildren(...lines.map(([label, said]) =>
          el(doc, 'div', { class: 'm-brand-line' }, el(doc, 'span', { class: 'm-brand-label' }, `${pad(label, LABEL)}${label ? ' : ' : '   '}`), said)))
      }
      const listed = JSON.stringify(entries)
      if (listed === drawnProjects) return
      drawnProjects = listed
      projects.replaceChildren(...entries.map((entry) => projectEl(doc, entry, open)))
      projects.hidden = entries.length === 0
      projects.style.setProperty('--m-brand-project-cols', String(Math.ceil(entries.length / PROJECT_ROWS) || 1))
      node.toggleAttribute('data-projects', entries.length > 0)
    },
  }
}

/** One project of the column: its key as the menu draws a key, its name dim — or marked, the one on screen. */
function projectEl(doc: Document, entry: ProjectEntry, open?: (entry: ProjectEntry) => void): HTMLElement {
  const node = el(doc, 'button', {
    type: 'button',
    class: 'm-brand-project',
    title: entry.path,
    'data-path': entry.path,
    'data-number': entry.number === null ? null : String(entry.number),
    'data-current': entry.current ? '' : null,
    'aria-current': entry.current ? 'page' : null,
    'aria-keyshortcuts': entry.number === null ? null : String(entry.number),
  }, el(doc, 'span', { class: 'm-brand-key' }, projectKey(entry.number)), el(doc, 'span', { class: 'm-brand-name' }, entry.name))
  node.addEventListener('click', () => {
    if (!entry.current) open?.(entry)
  })
  // The keys stay where they were: a click must not leave the focus on the button, where SPC would click it again.
  node.addEventListener('mousedown', (event) => event.preventDefault())
  return node
}

/**
 * The tip under the Version line: one at a time, the next every {@link TIP_MS}, from a place picked at random so each
 * opening starts somewhere else; a click opens the page with all of them. The turning is CSS, not a timer: every tip
 * stands in the same place and a keyframe shows each for its own {@link TIP_MS} of the round ({@link tipKeyframes}).
 * So a panel no one sees turns nothing, the pointer on it holds it still (shell.css), and without motion the first
 * tip stands alone. Screen readers hear the button's name, not the stack.
 */
function tipLine(doc: Document, tips: BrandTips): HTMLElement {
  const n = tips.tips.length
  const start = Math.floor(Math.random() * n)
  if (n > 1) tipKeyframes(doc, n)
  // Each tip carries its own label, so `Tip :` stands on the tip's first line however many lines it takes.
  const items = tips.tips.map((tip, i) => {
    const turn = (i - start + n) % n
    return el(doc, 'span', {
      class: 'm-brand-tip-item',
      'data-first': turn === 0 ? '' : null,
      style: n > 1 ? `animation: m-tip-${n} ${n * TIP_MS}ms linear ${turn * TIP_MS}ms infinite both;` : null,
    }, el(doc, 'span', { class: 'm-brand-label' }, `${pad('Tip', LABEL)} : `), el(doc, 'span', { class: 'm-brand-tip-said' }, tip))
  })
  const text = el(doc, 'button', { type: 'button', class: 'm-brand-tip-text', title: tips.label, 'aria-label': tips.label }, ...items)
  text.addEventListener('click', () => tips.open())
  // The keys stay where they were, as on a project (projectEl): the tips page hands the focus back to what held it, and
  // on this button SPC — the leader — would click it again and reopen the page instead of the menu.
  text.addEventListener('mousedown', (event) => event.preventDefault())
  // At the foot of the info column, so its last line stands on the tagline's row (shell.css).
  return el(doc, 'div', { class: 'm-brand-tip' }, text)
}

/**
 * The round for `n` tips, once per document: shown for one `n`th of it. It fades out while the next fades in, so the
 * label both carry at the same place stays on the screen. `both` fill keeps a tip whose turn has not come hidden until
 * it does.
 */
export function tipKeyframes(doc: Document, n: number): void {
  const id = `m-tip-${n}`
  if (doc.getElementById(id)) return
  const shown = 100 / n
  const fade = Math.min(shown / 4, (400 / (n * TIP_MS)) * 100)
  const at = (v: number) => `${Number(v.toFixed(3))}%`
  const style = el(doc, 'style', { id })
  style.textContent = `@keyframes ${id} { 0% { opacity: 0; } ${at(fade)} { opacity: 1; } ${at(shown)} { opacity: 1; } ${at(shown + fade)}, 100% { opacity: 0; } }`
  doc.head.append(style)
}
