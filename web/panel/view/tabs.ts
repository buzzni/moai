// Tab shell: the kanban / list / wiki tab strip (roving tabindex) and the panels they switch.
// Standalone: it takes props and callbacks and touches only the elements it makes. Where the last tab is
// remembered (the store, the saved settings) is the caller's business; normalizeTab() cleans what comes back.

import { el, icon } from "./components.ts"
import type { IconName } from "./icons.ts"

export type TabId = "board" | "list" | "wiki"

export interface TabDef {
  id: TabId
  icon: IconName
  /** Korean labels by default; pass your own through createTabs({ labels }). */
  label: string
}

export const TABS: ReadonlyArray<TabDef> = [
  { id: "board", icon: "kanban", label: "칸반" },
  { id: "list", icon: "list-tree", label: "목록" },
  { id: "wiki", icon: "book-open", label: "위키" },
]

export const DEFAULT_TAB: TabId = "list"

/** A stored or received value to a tab id; anything unknown falls back to the default. */
export function normalizeTab(value: unknown): TabId {
  return TABS.some((tab) => tab.id === value) ? (value as TabId) : DEFAULT_TAB
}

/** The tab a key moves to from `current`, or null when the key is not a tab-strip key. Wraps around. */
export function nextTab(current: TabId, key: string): TabId | null {
  const index = TABS.findIndex((tab) => tab.id === current)
  if (index < 0) return null
  const at = (i: number): TabId => TABS[i]!.id
  switch (key) {
    case "ArrowRight": return at((index + 1) % TABS.length)
    case "ArrowLeft": return at((index - 1 + TABS.length) % TABS.length)
    case "Home": return at(0)
    case "End": return at(TABS.length - 1)
    default: return null
  }
}

export const tabDomId = (id: TabId): string => `m-tab-${id}`
export const panelDomId = (id: TabId): string => `m-panel-${id}`

export interface TabsProps {
  selected: TabId
  onSelect: (id: TabId) => void
  labels?: Partial<Record<TabId, string>>
  /** Accessible name of the tablist. */
  ariaLabel?: string
}

export interface Tabs {
  el: HTMLDivElement
  selected(): TabId
  /** Update the strip (not the panels) and keep the roving tabindex in step. Does not call onSelect. */
  setSelected(id: TabId): void
  /** Unread count after the label; 0 or null removes it. */
  setBadge(id: TabId, count: number | null): void
}

export function createTabs(doc: Document, props: TabsProps): Tabs {
  const root = el(doc, "div", { class: "m-tablist", role: "tablist", "aria-label": props.ariaLabel ?? "Moai" })
  const buttons = new Map<TabId, HTMLButtonElement>()
  const names = new Map<TabId, string>()
  const badges = new Map<TabId, HTMLElement>()
  let selected = props.selected

  for (const tab of TABS) {
    const name = props.labels?.[tab.id] ?? tab.label
    const button = el(doc, "button", {
      type: "button",
      class: "m-tab",
      role: "tab",
      id: tabDomId(tab.id),
      "aria-controls": panelDomId(tab.id),
      "data-tab": tab.id,
      // The label is dropped below 380px (components.css) and `title` then names the tab; with a badge in it the
      // content would be the whole name, so setBadge names the tab itself.
      title: name,
    })
    button.append(icon(doc, tab.icon, 15), el(doc, "span", {}, name))
    button.addEventListener("click", () => choose(tab.id, false))
    buttons.set(tab.id, button)
    names.set(tab.id, name)
    root.append(button)
  }

  function paint(): void {
    for (const [id, button] of buttons) {
      const on = id === selected
      button.setAttribute("aria-selected", String(on))
      button.tabIndex = on ? 0 : -1
    }
  }

  function choose(id: TabId, focus: boolean): void {
    const changed = id !== selected
    selected = id
    paint()
    if (focus) buttons.get(id)?.focus()
    if (changed) props.onSelect(id)
  }

  root.addEventListener("keydown", (event) => {
    const target = nextTab(selected, event.key)
    if (!target) return
    event.preventDefault()
    choose(target, true)
  })
  paint()

  return {
    el: root,
    selected: () => selected,
    setSelected(id) {
      selected = id
      paint()
    },
    setBadge(id, count) {
      const button = buttons.get(id)
      const existing = badges.get(id)
      if (!count) {
        existing?.remove()
        badges.delete(id)
        button?.removeAttribute("aria-label")
      } else {
        const text = count > 99 ? "99+" : String(count)
        // Written only when it changes: the shell sets the badges on every render.
        const label = `${names.get(id) ?? ""} ${text}`.trim()
        if (button && button.getAttribute("aria-label") !== label) button.setAttribute("aria-label", label)
        if (existing) {
          if (existing.textContent !== text) existing.textContent = text
        } else {
          const badge = el(doc, "span", { class: "m-chip m-chip-new" }, text)
          badges.set(id, badge)
          button?.append(badge)
        }
      }
      // A count widens the strip: components.css lets the labels give way sooner while one shows.
      root.toggleAttribute("data-badged", badges.size > 0)
    },
  }
}

export interface TabPanels {
  el: HTMLDivElement
  panel(id: TabId): HTMLDivElement
  /** Show one panel and hide the rest, remembering each hidden panel's scroll offsets and restoring the shown one's. */
  show(id: TabId): void
  current(): TabId
}

/**
 * The panels the tabs switch. Every panel stays in the document, only hidden, so what a view built
 * (cursor, selection, wiki trail) survives a tab change. Scroll offsets do not survive display:none, so the
 * elements marked data-keep-scroll (and the panel itself) are saved on the way out and put back on the way in.
 * Offsets are kept per element, so a view that re-renders while hidden gets them back only on the elements
 * that are still there; new ones start at the top.
 */
export function createTabPanels(doc: Document, initial: TabId): TabPanels {
  const root = el(doc, "div", { class: "m-body" })
  const panels = new Map<TabId, HTMLDivElement>()
  const saved = new Map<TabId, Array<[HTMLElement, number, number]>>()
  let current = initial

  const scrollers = (panel: HTMLElement): HTMLElement[] => [panel, ...panel.querySelectorAll<HTMLElement>("[data-keep-scroll]")]

  for (const tab of TABS) {
    const panel = el(doc, "div", {
      class: "m-tabpanel",
      role: "tabpanel",
      id: panelDomId(tab.id),
      "aria-labelledby": tabDomId(tab.id),
      hidden: tab.id !== initial,
    })
    panels.set(tab.id, panel)
    root.append(panel)
  }

  return {
    el: root,
    panel: (id) => panels.get(id)!,
    current: () => current,
    show(id) {
      if (id === current) return
      const leaving = panels.get(current)!
      saved.set(current, scrollers(leaving).map((node) => [node, node.scrollTop, node.scrollLeft]))
      leaving.hidden = true
      const entering = panels.get(id)!
      entering.hidden = false
      for (const [node, top, left] of saved.get(id) ?? []) {
        if (!entering.contains(node)) continue
        node.scrollTop = top
        node.scrollLeft = left
      }
      current = id
    },
  }
}
