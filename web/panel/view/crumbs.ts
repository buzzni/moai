// The path line under the toolbar (design §1, TUI `crumbs`): where the list stands (`/milestone/epic`), a
// badge for what the view hides and a non-default order, and a warning badge for a search or filter that is
// on, with how to take it off. The pure functions build the line from the state; mountCrumbs draws it.
// A segment is a button: clicking it goes to that level.

import { BRANCH_GLYPH, worktreesOf } from "./branch.ts"
import { LOST, NO_MILESTONE, type IssueIndex, type Path, type Seg } from "../data/index.ts"
import type { Copy } from "../i18n/index.ts"
import { sameSort, type Sort } from "../data/query.ts"
import type { State } from "../store.ts"
import { chip, el } from "./components.ts"

export interface CrumbSegment {
  label: string
  /** Where clicking it goes. The first segment is the root, `[]`. */
  path: Path
}

/** What the path line reads from the state: the search text and the applied filter text hang as a badge. */
export type CrumbState = Pick<State, "path" | "view" | "sort" | "defaultSort" | "search" | "filter">

export interface CrumbLine {
  segments: CrumbSegment[]
  /** `done · deferred hidden · sort priority↑, updated↓`, or null when the view hides nothing and the order is the default (the saved one, if any). */
  look: string | null
  /** The search (`/text`) or filter on, or null. */
  hung: string | null
}

/** A segment's name: the row's title, or the bucket's name. A row the index does not hold shows its id. */
export function segLabel(seg: Seg, index: IssueIndex | null, copy: Copy): string {
  if (seg === NO_MILESTONE) return copy.noMilestone
  if (seg === LOST) return copy.lost
  const id = seg.slice(2)
  const at = index?.find(id)
  const title = at === undefined ? undefined : index!.rows[at]!.title
  return typeof title === "string" && title ? title : id
}

export function crumbSegments(path: Path, index: IssueIndex | null, copy: Copy): CrumbSegment[] {
  return [
    { label: copy.shell.rootSegment, path: [] },
    ...path.map((seg, n) => ({ label: segLabel(seg, index, copy), path: path.slice(0, n + 1) })),
  ]
}

/** `priority↑, updated↓` — the TUI's order words, and `(reversed)` when the whole list is turned. */
export function sortWords(sort: Sort, copy: Copy): string {
  const words = sort.fields.map((field) => `${copy.shell.sortFields[field.key]}${field.dir === "desc" ? "↓" : "↑"}`).join(", ")
  return sort.reversed ? `${words} ${copy.shell.reversed}` : words
}

/** What the view hides, then an order other than the default (the saved one, if any), as one badge; null when there is neither. */
export function lookBadge(state: Pick<CrumbState, "view" | "sort" | "defaultSort">, copy: Copy): string | null {
  const names = [...state.view.hiddenStatuses]
  if (state.view.hideDeferred) names.push(copy.shell.deferred)
  if (state.view.hideBacklog) names.push(copy.shell.backlog)
  const parts: string[] = []
  if (names.length) parts.push(copy.shell.hidden(names))
  if (!sameSort(state.sort, state.defaultSort)) parts.push(copy.shell.sorted(sortWords(state.sort, copy)))
  return parts.length ? parts.join(" · ") : null
}

/** The search or filter that narrows the rows now (their texts). A filter wins: the TUI hangs one at a time. */
export function hungBadge(state: { search: string; filter?: string }): string | null {
  const filter = state.filter?.trim()
  if (filter) return filter
  return state.search ? `/${state.search}` : null
}

export function crumbLine(state: CrumbState, index: IssueIndex | null, copy: Copy): CrumbLine {
  return { segments: crumbSegments(state.path, index, copy), look: lookBadge(state, copy), hung: hungBadge({ search: state.search.text, filter: state.filter.applied }) }
}

export interface Crumbs {
  el: HTMLElement
  render(state: CrumbState, index: IssueIndex | null): void
}

export function mountCrumbs(doc: Document, copy: Copy, enter: (path: Path) => void): Crumbs {
  const root = el(doc, "nav", { class: "m-crumbs", "aria-label": copy.shell.pathLabel })
  const trail = el(doc, "div", { class: "m-crumbs-path" })
  const badges = el(doc, "div", { class: "m-crumbs-badges" })
  root.append(trail, badges)
  let drawn = ""

  return {
    el: root,
    render(state, index) {
      const line = crumbLine(state, index, copy)
      // The worktrees whose rows stand overlaid, named as the TUI's path line names them (se-etwu).
      const trees = state.view.worktree && index ? worktreesOf(index.rows) : []
      // The line redraws on every store change; rebuilding it only when it reads differently keeps the
      // keyboard focus on a segment button across unrelated changes.
      const key = JSON.stringify([line, trees])
      if (key === drawn) return
      drawn = key
      const parts: Node[] = []
      line.segments.forEach((segment, n) => {
        if (n > 1) parts.push(el(doc, "span", { class: "m-crumbs-sep", "aria-hidden": "true" }, "/"))
        const last = n === line.segments.length - 1
        const button = el(doc, "button", {
          type: "button",
          class: "m-crumb",
          "aria-current": last ? "location" : null,
          "aria-label": n === 0 ? copy.root : null,
          title: segment.label,
        }, segment.label)
        button.addEventListener("click", () => enter(segment.path))
        parts.push(button)
      })
      trail.replaceChildren(...parts)
      const chips: Node[] = []
      if (trees.length) chips.push(el(doc, "span", { class: "m-branch m-crumbs-trees", title: trees.map((tree) => `${BRANCH_GLYPH} ${tree}`).join("\n") }, `${BRANCH_GLYPH} ${trees.join(", ")}`))
      if (line.look) chips.push(chip(doc, { label: line.look }))
      if (line.hung) chips.push(chip(doc, { label: line.hung, tone: "warn" }), el(doc, "span", { class: "m-crumbs-hint" }, copy.shell.clearHint))
      badges.replaceChildren(...chips)
    },
  }
}
