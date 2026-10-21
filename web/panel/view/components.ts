// Shared building blocks for the panel views. Each function takes a Document and plain props and returns
// an element carrying the m-* classes from style/components.css. No state, no store: callers wire events.
// Text always goes in through textContent, never innerHTML.

import { iconEl, type IconName } from "./icons.ts"

export interface Attrs {
  [name: string]: string | number | boolean | null | undefined
}

/** Create an element with attributes (false/null/undefined skip the attribute, true sets it empty). */
export function el<K extends keyof HTMLElementTagNameMap>(
  doc: Document,
  tag: K,
  attrs: Attrs = {},
  ...children: Array<Node | string | null | undefined>
): HTMLElementTagNameMap[K] {
  const node = doc.createElement(tag)
  for (const [name, value] of Object.entries(attrs)) {
    if (value === false || value === null || value === undefined) continue
    node.setAttribute(name, value === true ? "" : String(value))
  }
  for (const child of children) {
    if (child === null || child === undefined) continue
    node.append(typeof child === "string" ? doc.createTextNode(child) : child)
  }
  return node
}

export function icon(doc: Document, name: IconName, size = 16): SVGSVGElement {
  const svg = iconEl(doc, name, size)
  svg.classList.add("m-icon")
  return svg
}

export interface IconButtonProps {
  icon: IconName
  /** Required: both aria-label and title, so the button is named for screen readers and hover. */
  label: string
  pressed?: boolean
  disabled?: boolean
  onClick?: (event: MouseEvent) => void
}

/** 28x28 icon button (look-and-feel 5.2). */
export function iconButton(doc: Document, props: IconButtonProps): HTMLButtonElement {
  const button = el(doc, "button", {
    type: "button",
    class: "m-iconbtn",
    "aria-label": props.label,
    title: props.label,
    "aria-pressed": props.pressed === undefined ? null : String(props.pressed),
    disabled: props.disabled,
  })
  button.append(icon(doc, props.icon))
  if (props.onClick) button.addEventListener("click", props.onClick)
  return button
}

export type ButtonVariant = "primary" | "outline" | "ghost" | "danger"

export interface ButtonProps {
  label: string
  variant?: ButtonVariant
  small?: boolean
  icon?: IconName
  disabled?: boolean
  onClick?: (event: MouseEvent) => void
}

/**
 * Text button (look-and-feel 5.3). A screen should hold one primary at most. A button with an icon also
 * carries its label as aria-label and title, because a narrow toolbar hides the label text and leaves the icon.
 */
export function button(doc: Document, props: ButtonProps): HTMLButtonElement {
  const variant = props.variant ?? "outline"
  const node = el(doc, "button", {
    type: "button",
    class: `m-btn m-btn-${variant}${props.small ? " m-btn-sm" : ""}`,
    "aria-label": props.icon ? props.label : null,
    title: props.icon ? props.label : null,
    disabled: props.disabled,
  })
  if (props.icon) node.append(icon(doc, props.icon))
  node.append(el(doc, "span", { class: "m-btn-label" }, props.label))
  if (props.onClick) node.addEventListener("click", props.onClick)
  return node
}

export type ChipTone = "neutral" | "ok" | "warn" | "danger" | "accent" | "new"

export interface ChipProps {
  label: string
  tone?: ChipTone
  /** Makes it a filter chip: an x button with this accessible name. */
  removeLabel?: string
  onRemove?: () => void
}

export function chip(doc: Document, props: ChipProps): HTMLSpanElement {
  const tone = props.tone && props.tone !== "neutral" ? ` m-chip-${props.tone}` : ""
  const node = el(doc, "span", { class: `m-chip${tone}` }, props.label)
  if (props.removeLabel !== undefined) {
    const x = el(doc, "button", { type: "button", class: "m-chip-x", "aria-label": props.removeLabel, title: props.removeLabel })
    x.append(icon(doc, "x", 12))
    if (props.onRemove) x.addEventListener("click", props.onRemove)
    node.append(x)
  }
  return node
}

export interface EmptyProps {
  icon?: IconName
  title: string
  hint?: string
}

export function emptyState(doc: Document, props: EmptyProps): HTMLDivElement {
  const node = el(doc, "div", { class: "m-empty", role: "status" })
  if (props.icon) node.append(el(doc, "span", { class: "m-empty-icon" }, icon(doc, props.icon, 18)))
  node.append(el(doc, "div", { class: "m-empty-title" }, props.title))
  if (props.hint) node.append(el(doc, "div", {}, props.hint))
  return node
}

export type BannerTone = "info" | "warn" | "danger"

/** Danger banners are announced (role=alert); the rest are polite status text. */
export function banner(doc: Document, tone: BannerTone, text: string): HTMLDivElement {
  const cls = tone === "info" ? "m-banner" : `m-banner m-banner-${tone}`
  return el(doc, "div", { class: cls, role: tone === "danger" ? "alert" : "status" }, text)
}

export function kbd(doc: Document, key: string): HTMLElement {
  return el(doc, "kbd", { class: "m-kbd" }, key)
}

export interface SearchProps {
  placeholder: string
  label: string
  scopes?: ReadonlyArray<{ value: string; label: string }>
  scopeLabel?: string
  hint?: string
  invalid?: boolean
}

export interface SearchBox {
  el: HTMLLabelElement
  input: HTMLInputElement
  select: HTMLSelectElement | null
}

/** Search field (look-and-feel 5.4): magnifier, input, optional scope select and a key hint. */
export function searchBox(doc: Document, props: SearchProps): SearchBox {
  const input = el(doc, "input", {
    type: "text",
    placeholder: props.placeholder,
    "aria-label": props.label,
    "aria-invalid": props.invalid ? "true" : null,
    spellcheck: "false",
    autocomplete: "off",
  })
  const root = el(doc, "label", { class: "m-search" }, icon(doc, "search", 14), input)
  let select: HTMLSelectElement | null = null
  if (props.scopes?.length) {
    select = selectBox(doc, { label: props.scopeLabel ?? props.label, options: props.scopes })
    root.append(select)
  }
  if (props.hint) root.append(kbd(doc, props.hint))
  return { el: root, input, select }
}

export interface SelectProps {
  label: string
  options: ReadonlyArray<{ value: string; label: string }>
  value?: string
  invalid?: boolean
}

/** Native select styled like an input; the platform gives keyboard and a11y for free. */
export function selectBox(doc: Document, props: SelectProps): HTMLSelectElement {
  const select = el(doc, "select", { class: "m-select", "aria-label": props.label, "aria-invalid": props.invalid ? "true" : null })
  for (const option of props.options) select.append(el(doc, "option", { value: option.value }, option.label))
  if (props.value !== undefined) select.value = props.value
  return select
}

/** Text input styled to the app's input spec. */
export function textInput(doc: Document, attrs: { label: string; placeholder?: string; invalid?: boolean }): HTMLInputElement {
  return el(doc, "input", {
    type: "text",
    class: "m-input",
    "aria-label": attrs.label,
    placeholder: attrs.placeholder,
    "aria-invalid": attrs.invalid ? "true" : null,
  })
}

export interface MenuItem {
  id: string
  label: string
  icon?: IconName
  checked?: boolean
  disabled?: boolean
  hint?: string
}

/** A static menu surface (container + items). Opening, positioning and key handling belong to the caller. */
export function menu(doc: Document, items: ReadonlyArray<MenuItem | "separator">, onPick: (id: string) => void): HTMLDivElement {
  const root = el(doc, "div", { class: "m-menu", role: "menu" })
  for (const item of items) {
    if (item === "separator") {
      root.append(el(doc, "hr", { class: "m-menu-sep", role: "separator" }))
      continue
    }
    const role = item.checked === undefined ? "menuitem" : "menuitemcheckbox"
    const node = el(doc, "button", {
      type: "button",
      class: "m-menu-item",
      role,
      "aria-checked": item.checked === undefined ? null : String(item.checked),
      "aria-disabled": item.disabled ? "true" : null,
      "data-id": item.id,
    })
    if (item.icon) node.append(icon(doc, item.icon, 15))
    node.append(el(doc, "span", { style: "flex:1" }, item.label))
    if (item.hint) node.append(kbd(doc, item.hint))
    node.addEventListener("click", () => {
      if (!item.disabled) onPick(item.id)
    })
    root.append(node)
  }
  return root
}

/** A list row shell (28px). Pass the cells in order; the title cell ellipsises. */
export function listRow(doc: Document, opts: { selected?: boolean; dim?: boolean }, ...cells: Node[]): HTMLDivElement {
  const row = el(doc, "div", {
    class: `m-row${opts.dim ? " m-row-dim" : ""}`,
    role: "option",
    "aria-selected": opts.selected ? "true" : "false",
    tabindex: opts.selected ? 0 : -1,
  })
  row.append(...cells)
  return row
}
