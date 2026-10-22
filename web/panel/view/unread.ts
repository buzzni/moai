// The NEW mark (se-ehdi.zcx): the chip a list row, a board card and the detail wear, and the count the tabs show.
// Which rows are unread is data/read.ts (the CLI's verdict); this file only draws and counts it.

import type { Row } from "../data/index.ts"
import { unreadIds } from "../data/read.ts"
import { chip } from "./components.ts"

/** The NEW chip (look-and-feel: `--warn-soft` ground, `--warn` text). The label is the literal word the TUI uses. */
export const newChip = (doc: Document): HTMLSpanElement => {
  const node = chip(doc, { label: "NEW", tone: "new" })
  node.classList.add("moai-new")
  return node
}

/**
 * The unread count after the label of the two tabs that list rows (`tabs.setBadge`): every unread id of the rows the
 * views are given, which is what `SPC m a` writes (twin lines a merge left count once). It is the project's total, not
 * what the tab draws at the person's place (a row the view hides, another place, an epic the board has no card for
 * count too). null when there is none, and when the source does not say who is unread at all.
 */
export function unreadBadges(rows: readonly Row[]): Record<"board" | "list", number | null> {
  const count = new Set(unreadIds(rows)).size || null
  return { board: count, list: count }
}
