// Narrow-width rules. The panel lives in a tool tab that is at least 280px wide and usually around 500px,
// so layout decisions are made from widths here, in one place, and style/components.css carries the same
// numbers for the parts only CSS can do (container queries). tests/moai-shell.test.mjs checks they agree.

/** Under this width the detail is forced to the bottom and the position toggle is disabled. */
export const FORCE_BOTTOM_BELOW = 640

/** The toolbar folds sort, view and column controls into icon buttons under this width. */
export const TOOLBAR_COMPACT_BELOW = 500

/** Right-hand list columns in the order they disappear as the list narrows, with the width below which each goes. */
export const COLUMN_DROPS: ReadonlyArray<{ column: "created" | "updated" | "epic" | "assignee" | "tags"; below: number }> = [
  { column: "created", below: 520 },
  { column: "updated", below: 520 },
  { column: "epic", below: 460 },
  { column: "assignee", below: 400 },
  { column: "tags", below: 340 },
]

export type DetailPosition = "right" | "bottom" | "left" | "top"

/** The position actually used: the saved preference, except that a narrow panel always gets bottom. */
export function effectivePosition(width: number, preferred: DetailPosition): DetailPosition {
  return width < FORCE_BOTTOM_BELOW ? "bottom" : preferred
}

/** Whether the position toggle can be used at this width. */
export function canChoosePosition(width: number): boolean {
  return width >= FORCE_BOTTOM_BELOW
}

/** Of the columns the user turned on, the ones that still fit a list this wide. */
export function visibleColumns<T extends string>(width: number, enabled: ReadonlyArray<T>): T[] {
  const dropped = new Set<string>(COLUMN_DROPS.filter((entry) => width < entry.below).map((entry) => entry.column))
  return enabled.filter((column) => !dropped.has(column))
}

export function toolbarCompact(width: number): boolean {
  return width < TOOLBAR_COMPACT_BELOW
}

export const horizontal = (p: DetailPosition): boolean => p === "left" || p === "right"
export const first = (p: DetailPosition): boolean => p === "left" || p === "top"
export const nextPosition = (p: DetailPosition): DetailPosition => ({ right: "bottom", bottom: "left", left: "top", top: "right" } as const)[p]
