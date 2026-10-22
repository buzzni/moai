// SPC leader menu (keybindings.md 3.6): a which-key grid of the layer the key router has open.
// The view only reflects router.state().menu; opening, closing, going up and staying open after a toggle are
// the router's job. Clicking an item is router.press(item.key), exactly the key press.

import { TABLE, type KeyTableCopy } from "../i18n/index.ts"
import { MENU_CONTROLS, keyLabel, labelOf, menuLayer, type KeyContext, type MenuItem, type MenuLayer } from "../keys/table.ts"
import { el, kbd } from "./components.ts"

export interface MenuProps {
  /** A click on an item: pass router.press. Receives the item's key token. */
  press: (key: string) => void
  /** The words (copy.table); Korean when not given. */
  words?: KeyTableCopy
}

export interface MenuView {
  el: HTMLElement
  /** Redraw for the router's menu path (null = closed) and the current context. */
  update: (path: ReadonlyArray<string> | null, ctx: KeyContext) => void
}

/** 'SPC › 보기' for ['SPC','v']; the root reads the root title ('메뉴'). */
export function breadcrumb(layer: MenuLayer): string {
  return layer.path.length <= 1 ? layer.title : `SPC › ${layer.title}`
}

function itemEl(doc: Document, item: MenuItem, press: (key: string) => void): HTMLButtonElement {
  // Only a shown/hidden state is a checkbox; other states (sort direction, split side) are plain items with a label.
  const checkable = item.kind === "toggle" && (item.state?.id === "shown" || item.state?.id === "hidden")
  const node = el(doc, "button", {
    type: "button",
    class: "m-menu-item m-wk-item",
    role: checkable ? "menuitemcheckbox" : "menuitem",
    "aria-checked": checkable ? String(item.state!.id === "shown") : null,
    "data-key": item.key,
    "data-kind": item.kind,
    tabindex: -1,
  })
  // The cell is narrow and cuts a long label with an ellipsis: the title keeps the whole of it.
  node.append(kbd(doc, keyLabel(item.key)), el(doc, "span", { class: "m-wk-label", title: item.label }, item.label))
  // The state labels already carry their brackets: '[보임]'.
  if (item.state) node.append(el(doc, "span", { class: "m-wk-state" }, item.state.label))
  node.addEventListener("click", () => press(item.key))
  // Keep focus where it was so the panel's keys keep going to the router.
  node.addEventListener("mousedown", (event) => event.preventDefault())
  return node
}

export function createMenu(doc: Document, props: MenuProps): MenuView {
  const words = props.words ?? TABLE.ko
  const root = el(doc, "div", { class: "m-menu m-whichkey", role: "menu", hidden: true, "aria-label": words.root })
  const update = (path: ReadonlyArray<string> | null, ctx: KeyContext): void => {
    if (!path) {
      root.hidden = true
      root.replaceChildren()
      return
    }
    const layer = menuLayer(path, ctx, words)
    const grid = el(doc, "div", { class: "m-wk-grid" })
    for (const item of layer.items) grid.append(itemEl(doc, item, props.press))
    const footer = el(doc, "div", { class: "m-wk-footer" })
    for (const control of MENU_CONTROLS) {
      footer.append(el(doc, "span", { class: "m-wk-control" }, ...control.keys.map((k) => kbd(doc, keyLabel(k))), labelOf(control, ctx, words)))
    }
    root.setAttribute("aria-label", breadcrumb(layer))
    root.replaceChildren(el(doc, "div", { class: "m-wk-title" }, breadcrumb(layer)), grid, footer)
    root.hidden = false
  }
  return { el: root, update }
}
