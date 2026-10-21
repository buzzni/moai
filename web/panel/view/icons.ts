// Inline lucide icons (lucide-svelte 0.460.1, ISC). The panel CSP allows no external images or fonts,
// so each icon is its SVG body, drawn with stroke="currentColor". Only icons the panel uses live here.

export const ICONS = {
  "kanban": "<path d=\"M6 5v11\"/><path d=\"M12 5v6\"/><path d=\"M18 5v14\"/>",
  "list-tree": "<path d=\"M21 12h-8\"/><path d=\"M21 6H8\"/><path d=\"M21 18h-8\"/><path d=\"M3 6v4c0 1.1.9 2 2 2h3\"/><path d=\"M3 10v6c0 1.1.9 2 2 2h3\"/>",
  "book-open": "<path d=\"M12 7v14\"/><path d=\"M3 18a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1h5a4 4 0 0 1 4 4 4 4 0 0 1 4-4h5a1 1 0 0 1 1 1v13a1 1 0 0 1-1 1h-6a3 3 0 0 0-3 3 3 3 0 0 0-3-3z\"/>",
  "panel-left": "<rect width=\"18\" height=\"18\" x=\"3\" y=\"3\" rx=\"2\"/><path d=\"M9 3v18\"/>",
  "panel-bottom": "<rect width=\"18\" height=\"18\" x=\"3\" y=\"3\" rx=\"2\"/><path d=\"M3 15h18\"/>",
  "search": "<circle cx=\"11\" cy=\"11\" r=\"8\"/><path d=\"m21 21-4.3-4.3\"/>",
  "filter": "<polygon points=\"22 3 2 3 10 12.46 10 19 14 21 14 12.46 22 3\"/>",
  "arrow-up-down": "<path d=\"m21 16-4 4-4-4\"/><path d=\"M17 20V4\"/><path d=\"m3 8 4-4 4 4\"/><path d=\"M7 4v16\"/>",
  "eye": "<path d=\"M2.062 12.348a1 1 0 0 1 0-.696 10.75 10.75 0 0 1 19.876 0 1 1 0 0 1 0 .696 10.75 10.75 0 0 1-19.876 0\"/><circle cx=\"12\" cy=\"12\" r=\"3\"/>",
  "columns-3": "<rect width=\"18\" height=\"18\" x=\"3\" y=\"3\" rx=\"2\"/><path d=\"M9 3v18\"/><path d=\"M15 3v18\"/>",
  "refresh-cw": "<path d=\"M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8\"/><path d=\"M21 3v5h-5\"/><path d=\"M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16\"/><path d=\"M8 16H3v5\"/>",
  "circle-help": "<circle cx=\"12\" cy=\"12\" r=\"10\"/><path d=\"M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3\"/><path d=\"M12 17h.01\"/>",
  "chevron-right": "<path d=\"m9 18 6-6-6-6\"/>",
  "x": "<path d=\"M18 6 6 18\"/><path d=\"m6 6 12 12\"/>",
  "folder": "<path d=\"M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z\"/>",
  "corner-left-up": "<polyline points=\"14 9 9 4 4 9\"/><path d=\"M20 20h-7a4 4 0 0 1-4-4V4\"/>",
} as const

export type IconName = keyof typeof ICONS

const SVG_NS = "http://www.w3.org/2000/svg"

/** The icon as markup, for templates that build HTML strings. */
export function iconSvg(name: IconName, size = 16): string {
  return `<svg xmlns="${SVG_NS}" width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false">${ICONS[name]}</svg>`
}

/**
 * The icon as an element. The body is a constant above, never user text. It is parsed as SVG (XML) rather
 * than set as HTML, so the bundle holds no innerHTML at all — tests/web_bundle.rs checks that.
 */
export function iconEl(doc: Document, name: IconName, size = 16): SVGSVGElement {
  const Parser = doc.defaultView?.DOMParser ?? DOMParser
  const parsed = new Parser().parseFromString(iconSvg(name, size), "image/svg+xml")
  return doc.importNode(parsed.documentElement, true) as unknown as SVGSVGElement
}
