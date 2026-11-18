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
 * in the accent, bold, with a `▸` before the name (and `aria-current`), so the colour is not alone — and a click opens another. A narrow header
 * drops the logo first and then this column (shell.css).
 */
import { versionLine, type About, type VersionCopy } from '../data/about.ts'
import type { ProjectEntry } from '../data/projects.ts'
import { el } from './components.ts'

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

/** What stands before the name of the project on screen (the others get blanks the same wide). */
export const CURRENT = '▸ '

/** `<n> ` before a numbered project; the width of `<9> ` in spaces before one without a number (draw.rs `numbered`). */
export const projectKey = (number: number | null): string => (number === null ? ' '.repeat('<9> '.length) : `<${number}> `)

export function mountBrand(doc: Document, copy: VersionCopy, open?: (entry: ProjectEntry) => void): Brand {
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
  }, el(doc, 'span', { class: 'm-brand-key' }, projectKey(entry.number)),
    // The mark is a glyph, not only the accent: the others get its width in blanks so the names stand in one line.
    el(doc, 'span', { class: 'm-brand-here', 'aria-hidden': 'true' }, entry.current ? CURRENT : ' '.repeat(CURRENT.length)),
    el(doc, 'span', { class: 'm-brand-name' }, entry.name))
  node.addEventListener('click', () => {
    if (!entry.current) open?.(entry)
  })
  // The keys stay where they were: a click must not leave the focus on the button, where SPC would click it again.
  node.addEventListener('mousedown', (event) => event.preventDefault())
  return node
}
