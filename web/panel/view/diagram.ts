import mermaid from 'mermaid'
import { el } from './components.js'

interface Diagram { url: string; width: number }
const cache = new Map<string, Promise<Diagram>>()
let nextId = 0
let initializedTheme: 'dark' | 'default' | undefined
let turn: Promise<unknown> = Promise.resolve()
function themeOf(doc: Document): 'dark' | 'default' {
  const selected = doc.documentElement.dataset.theme
  return selected === 'dark' || (!selected && doc.defaultView?.matchMedia?.('(prefers-color-scheme: dark)').matches) ? 'dark' : 'default'
}
function render(doc: Document, source: string, theme: 'dark' | 'default'): Promise<Diagram> {
  const key = JSON.stringify([theme, source])
  const held = cache.get(key)
  if (held) { cache.delete(key); cache.set(key, held); return held }
  const job = turn.catch(() => {}).then(async () => {
    const measure = el(doc, 'div', { class: 'm-md-measure', 'aria-hidden': 'true' })
    doc.body.appendChild(measure)
    try {
      if (initializedTheme !== theme) {
        mermaid.initialize({ startOnLoad: false, securityLevel: 'strict', theme, htmlLabels: false, flowchart: { htmlLabels: false, useMaxWidth: false }, sequence: { useMaxWidth: false }, class: { useMaxWidth: false }, state: { useMaxWidth: false }, gantt: { useMaxWidth: false }, pie: { useMaxWidth: false } })
        initializedTheme = theme
      }
      const { svg } = await mermaid.render(`moai-diagram-${++nextId}`, source, measure)
      const viewBox = /viewBox=["']\s*[-\d.]+[ ,]+[-\d.]+[ ,]+([\d.]+)/i.exec(svg)
      const width = Number(viewBox?.[1]) || 300
      const bytes = new TextEncoder().encode(svg)
      let binary = ''
      for (const byte of bytes) binary += String.fromCharCode(byte)
      return { url: `data:image/svg+xml;base64,${btoa(binary)}`, width }
    } finally { measure.remove() }
  })
  turn = job
  cache.set(key, job)
  if (cache.size > 64) cache.delete(cache.keys().next().value!)
  job.catch(() => { if (cache.get(key) === job) cache.delete(key) })
  return job
}
export async function hydrateDiagrams(root: Element, doc: Document, words: { alt: string, error: string }): Promise<void> {
  if (!globalThis.moaiVendor?.mermaid || !mermaid) return
  const theme = themeOf(doc)
  await Promise.all([...root.querySelectorAll<HTMLElement>('figure.m-md-mermaid')].map(async figure => {
    const pre = figure.querySelector<HTMLElement>('pre')
    if (!pre) return
    try {
      const image = await render(doc, pre.textContent ?? '', theme)
      if (!figure.isConnected) return
      const img = el(doc, 'img', { class: 'm-md-diagram', src: image.url, width: String(image.width), alt: words.alt })
      figure.insertBefore(img, pre)
      pre.hidden = true
    } catch (error) {
      if (figure.isConnected) figure.appendChild(el(doc, 'p', { class: 'm-md-diagram-error' }, `${words.error}: ${String(error)}`))
    }
  }))
}
