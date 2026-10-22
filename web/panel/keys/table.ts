// Key table: every Moai panel key as data (docs/moai-panel/keybindings.md §3). Pure — no DOM.
// The router (router.ts) reads it to turn keys into action ids; the SPC menu grid (view/menu.ts) and the help
// overlay (view/help.ts) are built from it, so what the menu shows and what a key does cannot drift apart.
//
// Key notation (one token per key press, tokens joined by a space for a sequence):
//   - a printable character is itself; Shift is already in the character ('G', '?', 'P')
//   - named keys: SPC Enter Esc Tab Backspace Up Down Left Right Home End PageUp PageDown Delete
//   - modifiers prefix the token in the order C- (Ctrl), A- (Alt), S- (Shift, named keys only): 'C-d', 'S-Tab', 'A-Backspace'
//   - sequences: 'g g', 'C-w w', 'SPC v l'
// Cmd/Meta combinations are never ours (they belong to the host), so they have no notation.
//
// Exports for the views:
//   menuLayer(path, ctx)          → the SPC menu layer at `path` (['SPC'] or ['SPC','v']): title + visible items,
//                                   each with key, label, kind ('toggle' keeps the menu open, 'action' closes it,
//                                   'layer' goes one level down) and a state label ([보임]/[숨김], [오름]/[내림], …).
//   helpSections(mode, ctx, platform) → grouped rows { label, keys } for the help overlay; `available` marks rows
//                                   that do something right now.
//   keyLabel(notation)            → how to print a key or sequence in a <kbd> ('C-d' → 'Ctrl-d', 'g g' → 'gg', 'Up' → '↑').
//   MENU_CONTROLS                 → Esc / SPC / Backspace inside the menu, for the menu's footer hint.
// Clicking a menu item is the router's press(item.key) — the same path as pressing it.
// Labels are ids (KeyLabel) or functions of the words: both resolve through the i18n catalogue's `table` group
// (KeyTableCopy), passed as `words` to menuLayer and helpSections — Korean when a caller passes none.

import { TABLE, type KeyTableCopy } from "../i18n/index.ts"

export type Platform = "mac" | "other"

/** The router's modes. The SPC menu is not a mode: it is a layer over browse and wiki (see `menu` in KEY_TABLE). */
export type Mode = "browse" | "prompt" | "sorter" | "wiki"
export type TableName = Mode | "menu"

/** Verdicts of keybindings.md §3. 제외 keys are simply absent. */
export type Verdict = "같음" | "확인" | "대체" | "변경" | "추가"

export type SortField = "priority" | "created" | "updated" | "status" | "assignee" | "title" | "due"
export type CellField = "id" | "priority" | "assignee" | "created" | "updated" | "counts" | "tags" | "epic" | "names"

/**
 * What the table needs to know about the panel right now. The caller (store) fills it; every field is optional and a
 * missing field reads as the panel's default (detail shown at the bottom, list focus, nothing hidden).
 */
export interface KeyContext {
  mode: Mode
  /** Selected tab. */
  tab?: "list" | "board" | "wiki"
  /** Focused pane. Browse: 'list' | 'detail'. Wiki: 'list' | 'page'. */
  focus?: "list" | "detail" | "page"
  /** Detail pane shown (SPC v d). Default true. */
  detail?: boolean
  detailAt?: "left" | "bottom"
  /** The panel is narrower than 640px and the detail is forced to the bottom: SPC o d does nothing. */
  detailAtLocked?: boolean
  /** Body shown as raw markdown (SPC v r). */
  raw?: boolean
  hidden?: { deferred?: boolean; backlog?: boolean; archived?: boolean; worktree?: boolean; columns?: ReadonlyArray<boolean> }
  /** Status column names in order (labels of SPC v 1~9; columns beyond the list are not offered). */
  columns?: ReadonlyArray<string>
  /** The order's fields; `id` has no SPC s key but is in the order when the sort editor put it there. */
  sort?: ReadonlyArray<{ field: SortField | "id"; desc: boolean }>
  /** List columns shown (SPC c). A missing field reads as shown. */
  cells?: Partial<Record<CellField, boolean>>
  /**
   * The numbered projects' names by digit (`1`-`9` and `SPC p 1`-`9`, data/projects.ts numberedNames); a digit past
   * them does nothing, as the TUI's (keys.rs `Project(n)` past `Ctx::projects`).
   */
  projects?: ReadonlyArray<string>
}

export type StateId =
  | "shown" | "hidden" | "asc" | "desc" | "left" | "bottom" | "raw" | "rendered" | "list" | "board" | "wiki"

/** The words when a caller passes none. */
const DEFAULT_WORDS: KeyTableCopy = TABLE.ko

export type HelpGroupId =
  | "move" | "enter" | "focus" | "tab" | "project" | "search" | "other"
  | "view" | "sort" | "cell" | "option" | "menu" | "prompt" | "sorter" | "wiki"

/** The help overlay's group order; the titles are the words' `groups`. */
export const HELP_GROUPS: ReadonlyArray<HelpGroupId> = [
  "move", "enter", "wiki", "focus", "tab", "project", "search", "view", "sort", "cell", "option", "other", "menu", "prompt", "sorter",
]

/** A binding's words, by id in the catalogue's `labels`. */
export type KeyLabel =
  | "move.down" | "move.up" | "move.top" | "move.bottom" | "move.halfDown" | "move.halfUp" | "move.pageDown" | "move.pageUp"
  | "wiki.halfDown" | "wiki.halfUp"
  | "focus.next" | "focus.prev" | "focus.left" | "focus.right" | "focus.up" | "focus.down"
  | "enter" | "leave" | "expand" | "collapse" | "expandAll" | "search" | "clear" | "help"
  | "filter" | "tab.list" | "tab.board" | "tab.wiki"
  | "view.deferred" | "view.backlog" | "view.archived" | "view.worktree" | "view.all" | "view.allHelp" | "view.detail" | "view.raw" | "view.rawHelp"
  | "sort.edit" | "option.detailAt" | "option.detailAtHelp" | "project.jumpHelp" | "menu.close" | "menu.up"
  | "prompt.apply" | "prompt.cancel" | "prompt.next" | "prompt.prev" | "prompt.up" | "prompt.down" | "prompt.clear" | "prompt.deleteWord"
  | "sorter.down" | "sorter.up" | "sorter.lower" | "sorter.raise" | "sorter.toggle" | "sorter.flip" | "sorter.apply" | "sorter.default" | "sorter.cancel"
  | "wiki.enter" | "wiki.back" | "wiki.page" | "wiki.nextLink" | "wiki.prevLink" | "wiki.search" | "wiki.clear"

/** A label id, or words built from the catalogue and the context (status column names, a sort field's name). */
export type Label = KeyLabel | ((words: KeyTableCopy, ctx: KeyContext) => string)

/** The SPC layers by their key after SPC. */
export type LayerId = "g" | "p" | "v" | "s" | "c" | "o" | "w"

export interface Binding {
  /** Action id the router dispatches (with `args`). The store performs it. */
  action: string
  args?: Readonly<Record<string, string | number>>
  /** Key sequences on every platform. */
  keys: ReadonlyArray<string>
  /** Extra sequences only on macOS (Ctrl-w: Linux/Windows lose it to the host's tab-close). */
  macKeys?: ReadonlyArray<string>
  /** Label for the help overlay and the menu. */
  label: Label
  group: HelpGroupId
  /** Menu items only: 'toggle' keeps the SPC menu open after running, 'action' (default) closes it. */
  kind?: "toggle" | "action"
  /** A motion key: inside the SPC menu, when the layer has no item on it, it closes the menu and moves (menu.rs). */
  motion?: boolean
  /** Menu bindings only: the modes whose SPC menu carries the item. */
  modes?: ReadonlyArray<"browse" | "wiki">
  /** Available in this context. Unavailable items are hidden from the menu and their keys do nothing. */
  when?: (ctx: KeyContext) => boolean
  /** Current state for the menu's state label. */
  state?: (ctx: KeyContext) => StateId | null
  verdict?: Verdict
  /** false: left out of the help overlay (repeated rows such as SPC v 2~9). */
  help?: boolean
  /** Keys printed in the help overlay instead of `keys`. */
  helpKeys?: ReadonlyArray<string>
  /** Label printed in the help overlay instead of `label` (a row that stands for several bindings, a menu label
   *  shortened to fit the menu's cell). */
  helpLabel?: Label
}

export interface Layer {
  /** Full prefix, e.g. 'SPC v'. */
  prefix: string
  /** Its title is the words' `layers[id]`. */
  id: LayerId
  /** State shown on the layer's entry in the parent layer (SPC g shows the current tab). */
  state?: (ctx: KeyContext) => StateId | null
}

export type KeyTable = Readonly<Record<TableName, ReadonlyArray<Binding>>>

// ---------------------------------------------------------------- context readers

const detailShown = (c: KeyContext): boolean => c.detail !== false
const detailAt = (c: KeyContext): "left" | "bottom" => c.detailAt ?? "bottom"
const listFocus = (c: KeyContext): boolean => (c.focus ?? "list") === "list"
const shown = (hidden: boolean | undefined): StateId => (hidden ? "hidden" : "shown")

/** Focus moves: the wiki always has list | page side by side; browse needs the detail and only moves along its axis. */
function focusWhen(side: "next" | "prev" | "left" | "right" | "up" | "down"): (c: KeyContext) => boolean {
  return (c) => {
    if (c.mode === "wiki") return side === "next" || side === "prev" || side === "left" || side === "right"
    if (!detailShown(c)) return false
    if (side === "left" || side === "right") return detailAt(c) === "left"
    if (side === "up" || side === "down") return detailAt(c) === "bottom"
    return true
  }
}

// ---------------------------------------------------------------- browse (list / board / detail)

const MOTION: ReadonlyArray<Binding> = [
  { action: "move.down", keys: ["j", "Down"], label: "move.down", group: "move", motion: true },
  { action: "move.up", keys: ["k", "Up"], label: "move.up", group: "move", motion: true },
  { action: "move.top", keys: ["g g", "Home"], label: "move.top", group: "move", motion: true },
  { action: "move.bottom", keys: ["G", "End"], label: "move.bottom", group: "move", motion: true },
  { action: "move.halfDown", keys: ["C-d"], label: "move.halfDown", group: "move", motion: true, verdict: "확인" },
  { action: "move.halfUp", keys: ["C-u"], label: "move.halfUp", group: "move", motion: true, verdict: "확인" },
  { action: "move.pageDown", keys: ["C-f", "PageDown"], label: "move.pageDown", group: "move", motion: true, verdict: "확인" },
  { action: "move.pageUp", keys: ["C-b", "PageUp"], label: "move.pageUp", group: "move", motion: true, verdict: "확인" },
]

/**
 * The numbered projects (moai-o14f): a bare digit, as the TUI's header `<1>`-`<9>` (keys.rs `Project(n)`), and the same
 * under `SPC p`, whose items are the projects' names. One help row stands for all nine, and the bare and `SPC p` forms
 * of one digit share it (helpSections joins rows of one action). A digit past the projects that exist does nothing.
 */
function digitBindings(prefix = "", modes?: ReadonlyArray<"browse" | "wiki">): Binding[] {
  return Array.from({ length: 9 }, (_, i): Binding => ({
    action: "project.jump",
    args: { number: i + 1 },
    keys: [`${prefix}${i + 1}`],
    label: (_w, c) => c.projects?.[i] ?? String(i + 1),
    helpLabel: "project.jumpHelp",
    group: "project",
    ...(modes ? { modes } : {}),
    when: (c) => i < (c.projects?.length ?? 0),
    help: i === 0,
    helpKeys: i === 0 ? [`${prefix}1~9`] : undefined,
  }))
}

const BROWSE: ReadonlyArray<Binding> = [
  ...MOTION,
  { action: "enter", keys: ["Enter"], label: "enter", group: "enter", when: listFocus },
  { action: "leave", keys: ["Backspace"], label: "leave", group: "enter", when: listFocus },
  { action: "expand", keys: ["l", "Right"], label: "expand", group: "enter", motion: true, when: listFocus },
  { action: "collapse", keys: ["h", "Left"], label: "collapse", group: "enter", motion: true, when: listFocus },
  { action: "expandAll", keys: ["Tab"], label: "expandAll", group: "enter", when: (c) => listFocus(c) && c.tab !== "board" },
  ...digitBindings(),
  { action: "search.open", keys: ["/"], label: "search", group: "search" },
  { action: "clear", keys: ["Esc"], label: "clear", group: "search" },
  { action: "help", keys: ["?"], label: "help", group: "other", verdict: "추가" },
]

// ---------------------------------------------------------------- SPC menu (browse and wiki)

const BOTH = ["browse", "wiki"] as const
const BROWSE_ONLY = ["browse"] as const

const SORTS: ReadonlyArray<[string, SortField]> = [
  ["p", "priority"],
  ["c", "created"],
  ["u", "updated"],
  ["s", "status"],
  ["a", "assignee"],
  ["t", "title"],
  // The deadline is `w` (when), as the explorer's: `SPC s d` is a key it took out and leaves dead (tui/keys.rs).
  ["w", "due"],
]

const CELLS: ReadonlyArray<[string, CellField]> = [
  ["i", "id"],
  ["p", "priority"],
  ["a", "assignee"],
  ["c", "created"],
  ["u", "updated"],
  ["n", "counts"],
  ["t", "tags"],
  ["e", "epic"],
  ["h", "names"],
]

const columnBindings: Binding[] = Array.from({ length: 9 }, (_, i): Binding => ({
  action: "view.column",
  args: { index: i },
  keys: [`SPC v ${i + 1}`],
  label: (w, c) => c.columns?.[i] ?? w.column(i + 1),
  group: "view",
  kind: "toggle",
  modes: BROWSE_ONLY,
  when: (c) => i < (c.columns?.length ?? 0),
  state: (c) => shown(c.hidden?.columns?.[i]),
  help: i === 0,
  helpKeys: i === 0 ? ["SPC v 1~9"] : undefined,
  // The one help row stands for all nine: not the first column's name.
  helpLabel: i === 0 ? (w) => w.columns : undefined,
}))

const sortBindings: Binding[] = SORTS.flatMap(([key, field]): Binding[] => [
  {
    action: "sort.set",
    args: { field },
    keys: [`SPC s ${key}`],
    label: (w) => w.sortFields[field],
    group: "sort",
    kind: "toggle",
    modes: BROWSE_ONLY,
    // Only when the order is this one field: pressing again flips it (keys.rs Browse::state).
    state: (c) => {
      const only = c.sort?.length === 1 ? c.sort[0] : undefined
      return only && only.field === field ? (only.desc ? "desc" : "asc") : null
    },
  },
  {
    action: "sort.add",
    args: { field },
    keys: [`SPC s ${key.toUpperCase()}`],
    label: (w) => w.sortAdd(w.sortFields[field]),
    helpLabel: (w) => w.sortAddHelp(w.sortFields[field]),
    group: "sort",
    kind: "toggle",
    modes: BROWSE_ONLY,
    state: (c) => {
      const hit = c.sort?.find((s) => s.field === field)
      return hit ? (hit.desc ? "desc" : "asc") : null
    },
  },
])

const cellBindings: Binding[] = CELLS.map(([key, field]): Binding => ({
  action: "cell.toggle",
  args: { field },
  keys: [`SPC c ${key}`],
  label: (w) => w.cellFields[field],
  group: "cell",
  kind: "toggle",
  modes: BROWSE_ONLY,
  state: (c) => shown(c.cells?.[field] === false),
}))

const focusMenu = (action: string, key: string, label: KeyLabel, side: Parameters<typeof focusWhen>[0]): Binding => ({
  action,
  keys: [`SPC w ${key}`],
  label,
  group: "focus",
  // The wiki is list | page side by side: it has no up/down (its help must not list them either).
  modes: side === "up" || side === "down" ? BROWSE_ONLY : BOTH,
  when: focusWhen(side),
  verdict: "대체",
})

/** Every SPC binding. Order is menu order (like keys.rs: the table's order is the menu's order). */
const MENU: ReadonlyArray<Binding> = [
  { action: "search.open", keys: ["SPC /"], label: "search", group: "search", modes: BROWSE_ONLY, help: false },
  { action: "filter.open", keys: ["SPC f"], label: "filter", group: "search", modes: BROWSE_ONLY },
  // Before `SPC g`, as the TUI's table puts its `SPC p` (keys.rs). Only the jumps: registering is the TUI's.
  ...digitBindings("SPC p ", BOTH),
  { action: "tab.list", keys: ["SPC g l"], label: "tab.list", group: "tab", modes: BOTH },
  { action: "tab.board", keys: ["SPC g b"], label: "tab.board", group: "tab", modes: BOTH },
  { action: "tab.wiki", keys: ["SPC g w"], label: "tab.wiki", group: "tab", modes: BOTH },
  { action: "view.deferred", keys: ["SPC v l"], label: "view.deferred", group: "view", kind: "toggle", modes: BROWSE_ONLY, state: (c) => shown(c.hidden?.deferred) },
  { action: "view.backlog", keys: ["SPC v b"], label: "view.backlog", group: "view", kind: "toggle", modes: BROWSE_ONLY, state: (c) => shown(c.hidden?.backlog) },
  { action: "view.archived", keys: ["SPC v o"], label: "view.archived", group: "view", kind: "toggle", modes: BROWSE_ONLY, state: (c) => shown(c.hidden?.archived) },
  // The other worktrees' rows over the list, as the TUI's `SPC v w` (se-etwu); on when the page opens.
  { action: "view.worktree", keys: ["SPC v w"], label: "view.worktree", group: "view", kind: "toggle", modes: BROWSE_ONLY, state: (c) => shown(c.hidden?.worktree) },
  { action: "view.all", keys: ["SPC v a"], label: "view.all", helpLabel: "view.allHelp", group: "view", modes: BROWSE_ONLY },
  ...columnBindings,
  { action: "view.detail", keys: ["SPC v d"], label: "view.detail", group: "view", kind: "toggle", modes: BROWSE_ONLY, state: (c) => shown(!detailShown(c)) },
  {
    action: "view.raw",
    keys: ["SPC v r"],
    label: "view.raw",
    helpLabel: "view.rawHelp",
    group: "view",
    kind: "toggle",
    modes: BOTH,
    // The wiki body is markdown whether or not the issue detail is shown.
    when: (c) => c.mode === "wiki" || detailShown(c),
    state: (c) => (c.raw ? "raw" : "rendered"),
  },
  ...sortBindings,
  { action: "sort.edit", keys: ["SPC s e"], label: "sort.edit", group: "sort", modes: BROWSE_ONLY },
  ...cellBindings,
  {
    action: "option.detailAt",
    keys: ["SPC o d"],
    label: "option.detailAt",
    helpLabel: "option.detailAtHelp",
    group: "option",
    kind: "toggle",
    modes: BROWSE_ONLY,
    when: (c) => detailShown(c) && !c.detailAtLocked,
    state: (c) => detailAt(c),
    verdict: "변경",
  },
  focusMenu("focus.next", "w", "focus.next", "next"),
  focusMenu("focus.prev", "W", "focus.prev", "prev"),
  focusMenu("focus.left", "h", "focus.left", "left"),
  focusMenu("focus.down", "j", "focus.down", "down"),
  focusMenu("focus.up", "k", "focus.up", "up"),
  focusMenu("focus.right", "l", "focus.right", "right"),
  { action: "help", keys: ["SPC ?"], label: "help", group: "other", modes: BOTH, verdict: "추가" },
]

export const LAYERS: ReadonlyArray<Layer> = [
  { prefix: "SPC p", id: "p" },
  { prefix: "SPC g", id: "g", state: (c) => (c.mode === "wiki" ? "wiki" : c.tab === "board" ? "board" : c.tab ?? "list") },
  { prefix: "SPC v", id: "v" },
  { prefix: "SPC s", id: "s" },
  { prefix: "SPC c", id: "c" },
  { prefix: "SPC o", id: "o" },
  { prefix: "SPC w", id: "w" },
]

/** Keys the router handles itself while the SPC menu is open (menu.rs MENU + the opening key). */
export const MENU_CONTROLS: ReadonlyArray<Binding> = [
  { action: "menu.close", keys: ["Esc", "SPC"], label: "menu.close", group: "menu" },
  { action: "menu.up", keys: ["Backspace"], label: "menu.up", group: "menu" },
]

// ---------------------------------------------------------------- prompt, sorter, wiki

const PROMPT: ReadonlyArray<Binding> = [
  { action: "prompt.apply", keys: ["Enter"], label: "prompt.apply", group: "prompt" },
  { action: "prompt.cancel", keys: ["Esc"], label: "prompt.cancel", group: "prompt" },
  { action: "prompt.next", keys: ["Tab"], label: "prompt.next", group: "prompt" },
  { action: "prompt.prev", keys: ["S-Tab"], label: "prompt.prev", group: "prompt" },
  { action: "prompt.up", keys: ["Up"], label: "prompt.up", group: "prompt" },
  { action: "prompt.down", keys: ["Down"], label: "prompt.down", group: "prompt" },
  { action: "prompt.clear", keys: ["C-u"], label: "prompt.clear", group: "prompt", verdict: "확인" },
  // Ctrl-w closes the browser tab, so neither the window moves nor the word delete take it: SPC w moves the focus.
  { action: "prompt.deleteWord", keys: ["A-Backspace"], label: "prompt.deleteWord", group: "prompt", verdict: "대체" },
]

const SORTER: ReadonlyArray<Binding> = [
  { action: "sorter.down", keys: ["j", "Down"], label: "sorter.down", group: "sorter" },
  { action: "sorter.up", keys: ["k", "Up"], label: "sorter.up", group: "sorter" },
  { action: "sorter.lower", keys: ["J"], label: "sorter.lower", group: "sorter" },
  { action: "sorter.raise", keys: ["K"], label: "sorter.raise", group: "sorter" },
  { action: "sorter.toggle", keys: ["SPC"], label: "sorter.toggle", group: "sorter" },
  { action: "sorter.flip", keys: ["d"], label: "sorter.flip", group: "sorter" },
  { action: "sorter.apply", keys: ["Enter"], label: "sorter.apply", group: "sorter" },
  { action: "sorter.default", keys: ["D"], label: "sorter.default", group: "sorter" },
  { action: "sorter.cancel", keys: ["Esc"], label: "sorter.cancel", group: "sorter" },
]

/** The wiki's motions: its page list moves the cursor on C-d / C-u, so these do not say the cursor stays. */
const WIKI_MOTION: ReadonlyArray<Binding> = MOTION.map((b) =>
  b.action === "move.halfDown" ? { ...b, label: "wiki.halfDown" } : b.action === "move.halfUp" ? { ...b, label: "wiki.halfUp" } : b)

const WIKI: ReadonlyArray<Binding> = [
  ...WIKI_MOTION,
  ...digitBindings(),
  { action: "wiki.enter", keys: ["Enter"], label: "wiki.enter", group: "wiki" },
  { action: "wiki.back", keys: ["Backspace", "h", "Left"], label: "wiki.back", group: "wiki" },
  { action: "wiki.page", keys: ["l", "Right"], label: "wiki.page", group: "wiki", motion: true },
  { action: "wiki.nextLink", keys: ["Tab"], label: "wiki.nextLink", group: "wiki" },
  { action: "wiki.prevLink", keys: ["S-Tab"], label: "wiki.prevLink", group: "wiki" },
  { action: "wiki.search", keys: ["/"], label: "wiki.search", group: "search" },
  { action: "wiki.clear", keys: ["Esc"], label: "wiki.clear", group: "wiki", verdict: "변경" },
  { action: "help", keys: ["?"], label: "help", group: "other", verdict: "추가" },
]

export const KEY_TABLE: KeyTable = {
  browse: BROWSE,
  menu: MENU,
  prompt: PROMPT,
  sorter: SORTER,
  wiki: WIKI,
}

// ---------------------------------------------------------------- helpers

/** A sequence notation to its tokens: 'SPC v l' → ['SPC', 'v', 'l']. */
export function tokens(notation: string): string[] {
  return notation.split(" ").filter((t) => t !== "")
}

/** The binding's key sequences on this platform. */
export function bindingKeys(binding: Binding, platform: Platform): string[] {
  return platform === "mac" ? [...binding.keys, ...(binding.macKeys ?? [])] : [...binding.keys]
}

export function isAvailable(binding: Binding, ctx: KeyContext): boolean {
  if (binding.modes && !binding.modes.includes(ctx.mode as "browse" | "wiki")) return false
  return binding.when ? binding.when(ctx) : true
}

/** A label's words: an id looked up in the catalogue, or a function of the catalogue and the context. */
export function say(label: Label, ctx: KeyContext, words: KeyTableCopy = DEFAULT_WORDS): string {
  return typeof label === "function" ? label(words, ctx) : words.labels[label]
}

export function labelOf(binding: Binding, ctx: KeyContext, words: KeyTableCopy = DEFAULT_WORDS): string {
  return say(binding.label, ctx, words)
}

export function stateOf(binding: Binding, ctx: KeyContext, words: KeyTableCopy = DEFAULT_WORDS): { id: StateId; label: string } | null {
  const id = binding.state?.(ctx) ?? null
  return id ? { id, label: words.states[id] } : null
}

/** Bindings the router looks at in a mode: its own table, plus the SPC menu for browse and wiki. */
export function bindingsFor(mode: Mode, table: KeyTable = KEY_TABLE): Binding[] {
  const own = [...table[mode]]
  if (mode === "browse" || mode === "wiki") own.push(...table.menu.filter((b) => !b.modes || b.modes.includes(mode)))
  return own
}

const startsWith = (seq: ReadonlyArray<string>, prefix: ReadonlyArray<string>): boolean =>
  seq.length >= prefix.length && prefix.every((t, i) => seq[i] === t)

export type Lookup =
  | { kind: "run"; binding: Binding }
  | { kind: "pending" }
  | { kind: "unknown" }

/** keys.rs `lookup`: a longer sequence starting with `seq` → pending; an exact one → run; otherwise unknown. */
export function lookup(bindings: ReadonlyArray<Binding>, seq: ReadonlyArray<string>, platform: Platform): Lookup {
  let exact: Binding | undefined
  for (const binding of bindings) {
    for (const key of bindingKeys(binding, platform)) {
      const t = tokens(key)
      if (!startsWith(t, seq)) continue
      if (t.length > seq.length) return { kind: "pending" }
      exact ??= binding
    }
  }
  return exact ? { kind: "run", binding: exact } : { kind: "unknown" }
}

/** Some available binding lies under `prefix` (menu.rs `live`): only then is a layer offered or entered. */
export function live(bindings: ReadonlyArray<Binding>, prefix: ReadonlyArray<string>, ctx: KeyContext, platform: Platform): boolean {
  return bindings.some((b) => isAvailable(b, ctx) && bindingKeys(b, platform).some((k) => {
    const t = tokens(k)
    return t.length > prefix.length && startsWith(t, prefix)
  }))
}

export interface MenuItem {
  /** The key to press (or to pass to router.press on click). */
  key: string
  /** Item label; a layer item reads '+보기' (the layer's title). */
  label: string
  kind: "toggle" | "action" | "layer"
  state: { id: StateId; label: string } | null
  action?: string
  args?: Readonly<Record<string, string | number>>
}

export interface MenuLayer {
  path: string[]
  /** 'SPC v' */
  prefix: string
  /** The words' root title ('메뉴') at the root, the layer's title below. */
  title: string
  items: MenuItem[]
  /** The layer has a toggle: it stays open after a toggle, so the footer should say "Esc 닫기". */
  waits: boolean
}

export function layerOf(prefix: string): Layer | undefined {
  return LAYERS.find((l) => l.prefix === prefix)
}

function layerTitle(prefix: string, words: KeyTableCopy): string {
  const layer = layerOf(prefix)
  return layer ? words.layers[layer.id] : prefix
}

/**
 * The SPC menu layer at `path` (['SPC'] is the root) as it should be drawn now: only available items, in table order,
 * sub-layers only when something under them is available. Clicking an item = router.press(item.key).
 */
export function menuLayer(
  path: ReadonlyArray<string> | string,
  ctx: KeyContext,
  words: KeyTableCopy = DEFAULT_WORDS,
  table: KeyTable = KEY_TABLE,
): MenuLayer {
  const at = typeof path === "string" ? tokens(path) : [...path]
  const prefix = at.join(" ")
  const bindings = table.menu.filter((b) => isAvailable(b, ctx))
  const items: MenuItem[] = []
  const seen = new Set<string>()
  for (const binding of bindings) {
    for (const key of binding.keys) {
      const t = tokens(key)
      if (t.length <= at.length || !startsWith(t, at)) continue
      const next = t[at.length]!
      if (seen.has(next)) continue
      seen.add(next)
      if (t.length === at.length + 1) {
        items.push({
          key: next,
          label: labelOf(binding, ctx, words),
          kind: binding.kind ?? "action",
          state: stateOf(binding, ctx, words),
          action: binding.action,
          args: binding.args,
        })
      } else {
        const sub = [...at, next].join(" ")
        const layer = layerOf(sub)
        const id = layer?.state?.(ctx) ?? null
        items.push({ key: next, label: `+${layer ? words.layers[layer.id] : "…"}`, kind: "layer", state: id ? { id, label: words.states[id] } : null })
      }
    }
  }
  return {
    path: at,
    prefix,
    title: at.length <= 1 ? words.root : layerTitle(prefix, words),
    items,
    waits: items.some((i) => i.kind === "toggle"),
  }
}

export interface HelpRow {
  action: string
  args?: Readonly<Record<string, string | number>>
  label: string
  /** Key notations; print each with keyLabel(). */
  keys: string[]
  /** Does something in this context right now. */
  available: boolean
}

export interface HelpSection {
  id: HelpGroupId
  title: string
  rows: HelpRow[]
}

/** The help overlay for a mode: rows grouped by HELP_GROUPS order, platform keys only, empty groups left out. */
export function helpSections(
  mode: Mode,
  ctx: KeyContext,
  platform: Platform,
  words: KeyTableCopy = DEFAULT_WORDS,
  table: KeyTable = KEY_TABLE,
): HelpSection[] {
  const bindings = bindingsFor(mode, table)
  if (mode === "browse" || mode === "wiki") bindings.push(...MENU_CONTROLS)
  const sections: HelpSection[] = []
  for (const group of HELP_GROUPS) {
    const rows: HelpRow[] = []
    for (const binding of bindings) {
      if (binding.group !== group || binding.help === false) continue
      const keys = binding.helpKeys ? [...binding.helpKeys] : bindingKeys(binding, platform)
      if (keys.length === 0) continue
      // One row per action: the SPC w and Ctrl-w forms of a focus move share a row.
      const same = rows.find((r) => r.action === binding.action && JSON.stringify(r.args) === JSON.stringify(binding.args))
      const available = binding.group === "menu" || isAvailable(binding, { ...ctx, mode })
      if (same) {
        same.keys.push(...keys)
        same.available ||= available
        continue
      }
      rows.push({ action: binding.action, args: binding.args, label: say(binding.helpLabel ?? binding.label, ctx, words), keys, available })
    }
    if (rows.length > 0) sections.push({ id: group, title: words.groups[group], rows })
  }
  return sections
}

const KEY_NAMES: Readonly<Record<string, string>> = {
  Up: "↑",
  Down: "↓",
  Left: "←",
  Right: "→",
  Backspace: "Bksp",
  PageUp: "PgUp",
  PageDown: "PgDn",
}

/** A key or sequence for a <kbd>: 'C-d' → 'Ctrl-d', 'S-Tab' → 'Shift-Tab', 'g g' → 'gg', 'Up' → '↑', 'SPC w w' stays. */
export function keyLabel(notation: string): string {
  if (notation === "g g") return "gg"
  return tokens(notation)
    .map((token) => {
      const m = /^((?:[CAS]-)*)(.+)$/.exec(token)!
      const mods = m[1]!.replace(/C-/g, "Ctrl-").replace(/A-/g, "Alt-").replace(/S-/g, "Shift-")
      return mods + (KEY_NAMES[m[2]!] ?? m[2]!)
    })
    .join(" ")
}
