// List/detail split: the detail sits on any of the four sides, one 28px icon
// button flips it, a resizer drags or arrow-keys the ratio, and a narrow panel is forced to the bottom.
// Standalone: positions and ratios come in through props/setters and go out through callbacks, so the
// caller (store) owns persistence. Ratios are the detail's share of the container, per position.

import { canChoosePosition, effectivePosition, horizontal, first, nextPosition, type DetailPosition } from "./responsive.ts"
import { el, icon, iconButton } from "./components.ts"

export type { DetailPosition } from "./responsive.ts"

export const DEFAULT_RATIO: Readonly<Record<DetailPosition, number>> = { right: 0.45, bottom: 0.3, left: 0.45, top: 0.3 }
export const MIN_RATIO = 0.15
export const MAX_RATIO = 0.85

/** Smallest list (3 rows of 28px) and detail the split will leave, in px. */
const MIN_LIST_PX = 84
const MIN_DETAIL_PX: Record<DetailPosition, number> = { right: 240, bottom: 120, left: 240, top: 120 }
const MIN_LIST_WIDTH_PX = 160

export type Ratios = Record<DetailPosition, number>

/** A stored or received ratio to a usable one: finite and within 15-85%, else the position's default. */
export function normalizeRatio(value: unknown, position: DetailPosition): number {
  return typeof value === "number" && Number.isFinite(value) ? Math.min(MAX_RATIO, Math.max(MIN_RATIO, value)) : DEFAULT_RATIO[position]
}

/**
 * Clamp a ratio for a container of `total` px along the split axis: 15-85%, and the list and detail each keep
 * their minimum size when the container is big enough for both. When it is not, the percentage bounds win.
 */
export function clampRatio(ratio: number, total: number, position: DetailPosition): number {
  let lo = MIN_RATIO
  let hi = MAX_RATIO
  if (total > 0) {
    const detailMin = MIN_DETAIL_PX[position] / total
    const listMin = (horizontal(position) ? MIN_LIST_WIDTH_PX : MIN_LIST_PX) / total
    if (detailMin + listMin <= 1) {
      lo = Math.max(lo, detailMin)
      hi = Math.min(hi, 1 - listMin)
    }
  }
  if (lo > hi) return normalizeRatio(ratio, position)
  return Math.min(hi, Math.max(lo, normalizeRatio(ratio, position)))
}

/** The detail's ratio for a pointer at `offset` px inside a container of `total` px. */
export function ratioFromPointer(offset: number, total: number, position: DetailPosition): number {
  if (total <= 0) return DEFAULT_RATIO[position]
  const detailShare = first(position) ? offset / total : 1 - offset / total
  return clampRatio(detailShare, total, position)
}

export interface SplitProps {
  position: DetailPosition
  ratios?: Partial<Ratios>
  detailVisible?: boolean
  /** Fired after a drag or key press changes the ratio of a position. */
  onRatio?: (position: DetailPosition, ratio: number) => void
  labels?: { resizer: string }
}

export interface Split {
  el: HTMLDivElement
  /** Put the list and detail content into their panes. */
  list: HTMLDivElement
  detail: HTMLDivElement
  resizer: HTMLDivElement
  /** The width the panel has (the iframe's), which decides the forced position. Call on every resize. */
  setWidth(width: number): void
  /** The saved preference; the position shown is effectivePosition(width, preference). */
  setPosition(position: DetailPosition): void
  /** What is shown right now. */
  position(): DetailPosition
  preferred(): DetailPosition
  setRatio(position: DetailPosition, ratio: number): void
  ratio(position: DetailPosition): number
  setDetailVisible(visible: boolean): void
  destroy(): void
}

export function createSplit(doc: Document, props: SplitProps): Split {
  const ratios: Ratios = { right: normalizeRatio(props.ratios?.right, "right"), top: normalizeRatio(props.ratios?.top, "top"), left: normalizeRatio(props.ratios?.left, "left"), bottom: normalizeRatio(props.ratios?.bottom, "bottom") }
  let preferred = props.position
  let width = Infinity
  let shown: DetailPosition = effectivePosition(width, preferred)

  const list = el(doc, "div", { class: "m-split-list", "data-keep-scroll": true })
  const detail = el(doc, "div", { class: "m-split-detail", "data-keep-scroll": true })
  const resizer = el(doc, "div", {
    class: "m-split-resizer",
    role: "separator",
    tabindex: 0,
    "aria-label": props.labels?.resizer ?? "Resize",
    "aria-valuemin": Math.round(MIN_RATIO * 100),
    "aria-valuemax": Math.round(MAX_RATIO * 100),
  })
  const root = el(doc, "div", { class: "m-split", "data-detail": props.detailVisible === false ? "hidden" : "shown" }, list, resizer, detail)

  function axisTotal(): number {
    const rect = root.getBoundingClientRect()
    return horizontal(shown) ? rect.width : rect.height
  }

  function paint(): void {
    root.dataset.position = shown
    resizer.setAttribute("aria-orientation", horizontal(shown) ? "vertical" : "horizontal")
    resizer.setAttribute("aria-valuenow", String(Math.round(ratios[shown] * 100)))
    root.style.setProperty("--m-detail-size", `${(ratios[shown] * 100).toFixed(2)}%`)
  }

  function commit(position: DetailPosition, ratio: number): void {
    ratios[position] = clampRatio(ratio, position === shown ? axisTotal() : 0, position)
    paint()
    props.onRatio?.(position, ratios[position])
  }

  function onPointerDown(event: PointerEvent): void {
    // Primary button (or touch/pen contact) only: a right or middle click must not start a drag.
    if (event.button !== 0) return
    event.preventDefault()
    const position = shown
    resizer.setPointerCapture?.(event.pointerId)
    resizer.dataset.dragging = ""
    doc.body.classList.add("m-resizing")
    doc.body.dataset.resize = position
    const move = (ev: PointerEvent): void => {
      const rect = root.getBoundingClientRect()
      const total = horizontal(position) ? rect.width : rect.height
      const offset = horizontal(position) ? ev.clientX - rect.left : ev.clientY - rect.top
      ratios[position] = ratioFromPointer(offset, total, position)
      paint()
    }
    const end = (): void => {
      resizer.removeEventListener("pointermove", move)
      resizer.removeEventListener("pointerup", end)
      resizer.removeEventListener("pointercancel", end)
      delete resizer.dataset.dragging
      doc.body.classList.remove("m-resizing")
      delete doc.body.dataset.resize
      props.onRatio?.(position, ratios[position])
    }
    resizer.addEventListener("pointermove", move)
    resizer.addEventListener("pointerup", end)
    resizer.addEventListener("pointercancel", end)
  }

  function onKeyDown(event: KeyboardEvent): void {
    const grow = horizontal(shown) ? (first(shown) ? "ArrowRight" : "ArrowLeft") : (first(shown) ? "ArrowDown" : "ArrowUp")
    const shrink = horizontal(shown) ? (first(shown) ? "ArrowLeft" : "ArrowRight") : (first(shown) ? "ArrowUp" : "ArrowDown")
    // Arrow keys move the divider toward or away from the detail on each side.
    const step = event.shiftKey ? 0.1 : 0.02
    let next: number | null = null
    if (event.key === shrink) next = ratios[shown] - step
    else if (event.key === grow) next = ratios[shown] + step
    else if (event.key === "Home") next = MIN_RATIO
    else if (event.key === "End") next = MAX_RATIO
    if (next === null) return
    event.preventDefault()
    commit(shown, next)
  }

  resizer.addEventListener("pointerdown", onPointerDown)
  resizer.addEventListener("keydown", onKeyDown)
  paint()

  return {
    el: root,
    list,
    detail,
    resizer,
    setWidth(next) {
      width = next
      shown = effectivePosition(width, preferred)
      paint()
    },
    setPosition(position) {
      preferred = position
      shown = effectivePosition(width, preferred)
      paint()
    },
    position: () => shown,
    preferred: () => preferred,
    setRatio(position, ratio) {
      ratios[position] = normalizeRatio(ratio, position)
      paint()
    },
    ratio: (position) => ratios[position],
    setDetailVisible(visible) {
      root.dataset.detail = visible ? "shown" : "hidden"
    },
    destroy() {
      resizer.removeEventListener("pointerdown", onPointerDown)
      resizer.removeEventListener("keydown", onKeyDown)
    },
  }
}

export interface PositionToggleProps {
  position: DetailPosition
  width: number
  onToggle: (next: DetailPosition) => void
  labels: { toRight?: string; toTop?: string; toLeft: string; toBottom: string; forcedBottom: string }
}

export interface PositionToggle {
  el: HTMLButtonElement
  /** Re-label for the position shown and the width; a narrow panel disables the button and says why. */
  update(position: DetailPosition, width: number): void
}

/**
 * The one 28px icon button that cycles the detail right, bottom, left and top. Its icon shows where the detail
 * is now; its name says what pressing does. Under the forced-bottom width it is disabled with the reason.
 */
export function createPositionToggle(doc: Document, props: PositionToggleProps): PositionToggle {
  let position = props.position
  const button = iconButton(doc, { icon: "panel-bottom", label: props.labels.toLeft })
  const apply = (width: number): void => {
    const free = canChoosePosition(width)
    const label = !free ? props.labels.forcedBottom : ({ right: props.labels.toRight ?? "Move details to the right", bottom: props.labels.toBottom, left: props.labels.toLeft, top: props.labels.toTop ?? "Move details to the top" })[nextPosition(position)]
    // aria-disabled, not disabled: a disabled button shows no title tooltip, and the tooltip is the explanation.
    if (free) button.removeAttribute("aria-disabled")
    else button.setAttribute("aria-disabled", "true")
    button.setAttribute("aria-label", label)
    button.title = label
    button.querySelector("svg")?.replaceWith(icon(doc, `panel-${position}`))
  }
  button.addEventListener("click", () => {
    if (button.getAttribute("aria-disabled") === "true") return
    props.onToggle(nextPosition(position))
  })
  apply(props.width)
  return {
    el: button,
    update(next, width) {
      position = next
      apply(width)
    },
  }
}
