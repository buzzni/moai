// The zone picker (`SPC o t`), as the TUI's (src/tui/zones.rs `Zones`, src/tui/draw.rs `zone_pick`): a filter box over
// the names, the one in use marked `now`. It opens a list to pick from; it does not cycle.
//
// - The first line clears the page's choice: the TUI's zone follows when it picked one (`/api/boot`'s `timezone`), else
//   the browser's — the line names which. Then `UTC`, then every name the browser knows (data/zone.ts zoneNames).
// - The cursor opens on the line in use. A zone in use the list does not hold aims at no line (zones.rs `aimed`): Enter
//   then keeps it, and a line under the box says so (`tui.tz.unlisted`).
// - Typing filters, case aside, and aims the cursor (zones.rs `key`). Only ↑ ↓ PgUp PgDn move it: j and k are letters
//   here, as in the TUI — a box that took them could not filter for `Tokyo` or `Jakarta`.
// - Enter picks the line under the cursor, a click picks the line clicked, Esc or a click outside closes without a change.
//
// It is a modal dialog (aria-modal): the key router leaves the keys pressed in it alone (keys/router.ts inModal).
// The look is the sort editor's popover (`.m-menu.m-sort-pop`, its `.m-menu-item[data-cursor]`). It never writes to
// the server: what is picked goes to the store (keys/wiring.ts), which the browser remembers (data/memory.ts).

import { fallbackZone, filterZones, validZone } from "../data/zone.ts"
import type { ZoneCopy } from "../i18n/index.ts"
import { el } from "./components.ts"

/** The line that clears the choice: no name of its own. */
const DEFAULT = ""
/** One page of ↑ ↓ PgUp PgDn, the TUI's fixed step (scroll.rs `PAGE`). */
const PAGE = 10

export interface ZonePickerProps {
  words: ZoneCopy
  /** Every name after the first line, `UTC` first (data/zone.ts zoneNames). */
  names: () => string[]
  /** A line was picked: a name, or null for the first line (the TUI's zone, else the browser's). */
  pick: (zone: string | null) => void
  /**
   * The picker closed, picked or not: the keys go back to the pane — unless the focus left it for something the person
   * chose (`away`: a click on the search box, a Tab), where it stays.
   */
  closed: (away: boolean) => void
}

export interface ZoneOpen {
  /** The zone picked on this page; null when none is. */
  chosen: string | null
  /** `/api/boot`'s `timezone`. */
  tui: string | null
}

export interface ZonePicker {
  el: HTMLElement
  input: HTMLInputElement
  open(now: ZoneOpen): void
  /** Closes without a change. */
  close(): void
  isOpen(): boolean
  /** The lines shown now, by their name ('' is the first line). */
  shown(): string[]
  /** The line under the cursor, or null when it aims at none. */
  aimed(): string | null
}

let uid = 0

export function createZonePicker(doc: Document, props: ZonePickerProps): ZonePicker {
  const { words } = props
  const id = `moai-zones-${++uid}`
  const input = el(doc, "input", {
    type: "text", class: "m-input m-zone-filter", placeholder: words.filter, "aria-label": words.filter,
    role: "combobox", "aria-expanded": "true", "aria-controls": `${id}-list`, autocomplete: "off", spellcheck: "false",
  })
  const list = el(doc, "div", { class: "m-zone-list", id: `${id}-list`, role: "listbox", "aria-label": words.title })
  const note = el(doc, "div", { class: "m-sort-note m-zone-note", role: "status" })
  const root = el(doc, "div", {
    class: "m-menu m-sort-pop m-zone-pop", role: "dialog", "aria-modal": "true", "aria-label": words.title, hidden: true,
  }, el(doc, "div", { class: "m-wk-title" }, words.title), input, list, note)

  let all: string[] = []
  let now: ZoneOpen = { chosen: null, tui: null }
  /** The name in use: the picked one as Intl spells it, or '' for the first line. */
  let inUse = DEFAULT
  let hits: string[] = []
  let cursor = 0
  let aimed = false

  const firstLine = (): string => {
    const fallback = fallbackZone(now.tui)
    return (fallback.from === "tui" ? words.tui : words.browser).replace("{zone}", fallback.zone)
  }

  /** The lines for the text typed: the first line only while nothing is typed (it has no name to match). */
  function settle() {
    const typed = input.value
    hits = typed.trim() === "" ? [DEFAULT, ...all] : filterZones(all, typed)
    // Never above the list's top (zones.rs saturates): a cursor of -1 would aim at no line while it says it aims.
    cursor = Math.max(0, Math.min(cursor, hits.length - 1))
  }

  const selected = (): number | null => (aimed && hits.length ? Math.min(cursor, hits.length - 1) : null)

  function draw() {
    const at = selected()
    list.replaceChildren(...hits.map((name, n) => {
      const line = el(doc, "div", {
        class: "m-menu-item m-zone-item", role: "option", id: `${id}-${n}`, "data-zone": name,
        "aria-selected": String(n === at), "data-cursor": n === at ? "" : null,
      }, el(doc, "span", { class: "m-zone-name" }, name === DEFAULT ? firstLine() : name))
      // The name in use carries a word, not a colour alone (the TUI's `now`).
      if (name === inUse) line.append(el(doc, "span", { class: "m-wk-state m-zone-now" }, words.now))
      return line
    }))
    if (at === null) input.removeAttribute("aria-activedescendant")
    else input.setAttribute("aria-activedescendant", `${id}-${at}`)
    const said = hits.length === 0 ? words.noMatch : at === null && inUse !== DEFAULT ? words.unlisted(inUse) : ""
    note.textContent = said
    note.hidden = said === ""
    const line = at === null ? null : list.children[at]
    ;(line as HTMLElement | null)?.scrollIntoView?.({ block: "nearest" })
  }

  function step(by: number, page = false) {
    if (!aimed) {
      // A cursor that aimed at nothing lands on the first line, as from the empty place above the list (zones.rs step).
      cursor = page && by > 0 ? Math.max(0, Math.min(hits.length - 1, PAGE)) : 0
    } else {
      cursor = Math.max(0, Math.min(hits.length - 1, cursor + by))
    }
    aimed = hits.length > 0
    draw()
  }

  function finish(zone: string | null | undefined, away = false) {
    if (root.hidden) return
    root.hidden = true
    list.replaceChildren()
    if (zone !== undefined) props.pick(zone)
    props.closed(away)
  }

  input.addEventListener("input", () => {
    // Typing is setting out to pick: the cursor aims (zones.rs `key`), only when the text changed.
    aimed = true
    settle()
    draw()
  })
  input.addEventListener("keydown", (event) => {
    if (event.isComposing) return
    const plain = !event.ctrlKey && !event.altKey && !event.metaKey
    let used = true
    if (plain && event.key === "ArrowDown") step(1)
    else if (plain && event.key === "ArrowUp") step(-1)
    else if (plain && event.key === "PageDown") step(PAGE, true)
    else if (plain && event.key === "PageUp") step(-PAGE, true)
    else if (event.key === "Enter") {
      const at = selected()
      // Aiming at nothing, Enter keeps the zone in use (zones.rs `at` is None).
      finish(at === null ? undefined : hits[at] === DEFAULT ? null : hits[at]!)
    } else if (event.key === "Escape" || event.key === "Esc") finish(undefined)
    else used = false
    if (used) {
      event.preventDefault()
      event.stopPropagation()
    }
  })
  // A click anywhere in the picker but the filter box (a line, the title, the note, the padding) leaves the caret in the
  // box: none of them takes the focus, and the box losing it would close the picker and drop what was typed.
  root.addEventListener("mousedown", (event) => {
    if (event.target !== input) event.preventDefault()
  })
  list.addEventListener("click", (event) => {
    const line = (event.target as Element | null)?.closest?.<HTMLElement>(".m-zone-item")
    if (!line) return
    const name = line.dataset.zone ?? DEFAULT
    finish(name === DEFAULT ? null : name)
  })
  // A click or a Tab out of the picker closes it without a change. The focus is on its way to `to`, the person's
  // choice: `away` keeps the pane from taking it back from inside this focusout (focusing there cancels the move).
  root.addEventListener("focusout", (event) => {
    const to = (event as FocusEvent).relatedTarget as Node | null
    if (to && root.contains(to)) return
    finish(undefined, to !== null)
  })

  return {
    el: root,
    input,
    open(state) {
      now = state
      all = props.names()
      inUse = validZone(state.chosen) ?? DEFAULT
      input.value = ""
      settle()
      const at = hits.indexOf(inUse)
      aimed = at >= 0
      cursor = Math.max(0, at)
      root.hidden = false
      draw()
      input.focus({ preventScroll: true })
    },
    close: () => finish(undefined),
    isOpen: () => !root.hidden,
    shown: () => [...hits],
    aimed: () => {
      const at = selected()
      return at === null ? null : hits[at]!
    },
  }
}
