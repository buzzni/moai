/**
 * The tips page (se-irhr): things to say to an agent session, by kind, each with a Copy button. It opens from the tip
 * the header turns through (view/brand.ts) and looks like the shortcut help (view/help.ts): a dialog over the panel
 * that keeps the keys while it is open, so `j` or Space typed at it do not move the board behind it. Tab goes round
 * its own controls; Esc and Close shut it and give the focus back.
 */
import type { TipsCopy } from '../i18n/index.ts'
import { isEditableTarget } from '../keys/router.ts'
import { button, el, iconButton } from './components.ts'
import { ACTS_BEHIND, EDITS_INPUT } from './help.ts'

export interface TipsView {
  el: HTMLElement
  open(): void
  close(): void
  isOpen(): boolean
}

/** Every tip in the page's order: the header turns through these. */
export const allTips = (words: TipsCopy): string[] => words.groups.flatMap((group) => group.tips)

/**
 * Put `text` on the clipboard and say so on `label`; where the frame may not use the clipboard, select `node`'s text for
 * the person to copy.
 */
export function copyText(doc: Document, text: string, node: HTMLElement, label: Element | null, copied: string): void {
  const win = doc.defaultView
  const select = () => {
    const range = doc.createRange()
    range.selectNodeContents(node)
    win?.getSelection()?.removeAllRanges()
    win?.getSelection()?.addRange(range)
  }
  const clipboard = win?.navigator.clipboard
  if (!clipboard) return select()
  clipboard.writeText(text).then(() => void (label && (label.textContent = copied)), select)
}

const FOCUSABLE = 'button, [href], [tabindex]:not([tabindex="-1"])'

export function createTips(doc: Document, words: TipsCopy, platform: 'mac' | 'other'): TipsView {
  const root = el(doc, 'div', { class: 'm-menu m-help m-tips', role: 'dialog', 'aria-modal': 'true', 'aria-label': words.title, tabindex: -1, hidden: true })
  let returnTo: Element | null = null

  const close = (): void => {
    if (root.hidden) return
    doc.removeEventListener('keydown', guard, true)
    root.hidden = true
    root.replaceChildren()
    const back = returnTo as HTMLElement | null
    returnTo = null
    if (back && back !== doc.body && back.isConnected) back.focus?.()
  }

  /**
   * While it is open nothing behind it takes a key (help.ts's guard, in the capture phase on the document): a key with
   * the focus outside brings the focus back, Esc closes, Tab goes round the page's own controls. The host's
   * combinations (copy, find, a tab switch; Cmd, and Ctrl off macOS) go on — but not to the key router behind, which
   * would run the panel's own on them (`C-d`, `C-f` move the list under the page; help.ts keeps them out through the
   * router's intercept). Stopped all the same, as help.ts stops them: a key a control behind acts on whatever the
   * modifier (ACTS_BEHIND) and a paste, cut, undo or redo into an input behind (EDITS_INPUT).
   */
  function guard(event: KeyboardEvent): void {
    if (root.hidden) return
    const host = event.metaKey || (event.ctrlKey && !event.altKey && platform !== 'mac')
    const edits = EDITS_INPUT.has(event.key.toLowerCase()) && isEditableTarget(event.target)
    if (host && !ACTS_BEHIND.has(event.key) && !edits) {
      event.stopPropagation()
      return
    }
    const target = event.target as Node | null
    const inside = target !== null && root.contains(target)
    if (event.key === 'Escape' || event.key === 'Esc') {
      event.preventDefault()
      event.stopPropagation()
      return close()
    }
    // Enter and Space reach the focused button; the page's own listener below keeps every key from the key router.
    if (inside && event.key !== 'Tab') return
    event.preventDefault()
    event.stopPropagation()
    if (!inside) return root.focus?.()
    const stops = [root, ...root.querySelectorAll<HTMLElement>(FOCUSABLE)]
    const at = stops.indexOf(doc.activeElement as HTMLElement)
    const next = stops[(at + (event.shiftKey ? -1 : 1) + stops.length) % stops.length]!
    next.focus?.()
  }

  // The key router listens on the document as a key bubbles up (keys/router.ts): a key pressed in the page stops here.
  root.addEventListener('keydown', (event) => event.stopPropagation())

  const open = (): void => {
    if (root.hidden) {
      returnTo = doc.activeElement
      doc.addEventListener('keydown', guard, true)
    }
    const head = el(doc, 'div', { class: 'm-help-head' }, el(doc, 'strong', {}, words.title), iconButton(doc, { icon: 'x', label: words.close, onClick: close }))
    const body = el(doc, 'div', { class: 'm-help-body' }, el(doc, 'p', { class: 'm-tips-intro' }, words.intro))
    for (const group of words.groups) {
      const section = el(doc, 'div', { class: 'm-help-section', role: 'group', 'aria-label': group.title }, el(doc, 'h3', { class: 'm-help-title' }, group.title))
      for (const tip of group.tips) {
        const said = el(doc, 'span', { class: 'm-tips-text' }, tip)
        const copy = button(doc, {
          label: words.copy,
          small: true,
          variant: 'ghost',
          onClick: () => copyText(doc, tip, said, copy.querySelector('.m-btn-label'), words.copied),
        })
        section.append(el(doc, 'div', { class: 'm-tips-row' }, said, copy))
      }
      body.append(section)
    }
    root.replaceChildren(head, body)
    root.hidden = false
    root.focus?.()
  }

  return { el: root, open, close, isOpen: () => !root.hidden }
}
