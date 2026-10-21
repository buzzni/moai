// Key router: keydown → key token → sequence buffer → action id (docs/moai-panel/keybindings.md §2, keys.rs Chord,
// menu.rs feed). Pure: createKeyRouter() knows no DOM; attachKeyRouter() is the thin adapter that wires a document.
//
// Rules:
//   - isComposing / keyCode 229 (an IME at work, Korean) → nothing at all in an editable target or the prompt, the
//     buffer is left alone; on any other target only a letter or digit event.code names goes on (codeKey).
//   - Cmd/Meta combinations and lone modifier keys are not ours (the host's shortcuts; Shift before 'G' must not
//     drop a pending 'g').
//   - prompt mode: only the prompt table (Enter, Esc, Tab, …); other keys type into the input.
//   - an editable target (input, textarea, select, contenteditable) outside prompt mode: nothing, and any pending
//     sequence or open menu is dropped.
//   - browse/wiki: Space, Backspace and '/' always get preventDefault (page scroll, back). Tab is prevented only
//     when an action used it (expandAll, a wiki link); otherwise it moves the focus as the browser does.
//   - an action whose handler did not use the key (dispatch returned false: the wiki search's Tab and ↑) is not
//     handled and its default is not prevented.
//   - sequences ('g g', 'C-w w'): a key that continues no sequence drops the buffer and is swallowed too (keys.rs
//     Chord::feed — "g j" does not move down).
//   - SPC menu (buffer starting with SPC): Esc or SPC closes, Backspace goes up one layer (closing at the root), an
//     unknown or unavailable key is ignored, a toggle item keeps the menu open, an action item closes it, a motion key
//     the layer does not carry closes the menu and moves. Auto-repeats of SPC or of a key inside the menu are ignored.
//   - a key pressed inside an open modal dialog (the shortcut help) is the dialog's (attachKeyRouter).
//   - Enter or Space on a focused button, link, checkbox, radio or summary is the control's own (its click), unless
//     a sequence or the SPC menu is held (attachKeyRouter). Tab and Shift-Tab there move the focus on. Space on a
//     tab of the tab strip (which selects as the focus moves) is the router's: SPC opens the menu there.
//   - a change of mode with a sequence or menu held (a click into an input, onto another tab) drops it: settle().

import {
  KEY_TABLE,
  bindingKeys,
  bindingsFor,
  isAvailable,
  live,
  lookup,
  type Binding,
  type KeyContext,
  type KeyTable,
  type Mode,
  type Platform,
} from "./table.ts"

export interface KeyEventLike {
  key: string
  ctrlKey?: boolean
  metaKey?: boolean
  altKey?: boolean
  shiftKey?: boolean
  isComposing?: boolean
  /** 229 while an IME is busy (Chrome reports it before isComposing turns true). */
  keyCode?: number
  /** Physical key ('KeyJ', 'Digit1'): the stand-in for a letter an input source (Korean) turned into a jamo. */
  code?: string
  /** An auto-repeat of a held key. */
  repeat?: boolean
  /** The event target is editable. When absent, `target` is checked with isEditableTarget(). */
  editable?: boolean
  target?: unknown
  /** The DOM event itself (attachKeyRouter fills it), for a dispatch that hands the key on (the wiki's handleKey). */
  source?: unknown
}

export interface DispatchedAction {
  id: string
  args?: Readonly<Record<string, string | number>>
  /** The key sequence that ran it, e.g. ['SPC', 'v', 'l']. */
  keys: string[]
  mode: Mode
  /** Ran from inside the SPC menu. */
  fromMenu: boolean
}

export interface KeyResult {
  /** The router consumed the key (ran an action, extended a sequence, or swallowed it). */
  handled: boolean
  preventDefault: boolean
  action?: DispatchedAction
}

export interface RouterState {
  /** Keys held for a sequence outside the menu, e.g. ['g'] or ['C-w']. */
  pending: string[]
  /** Open SPC menu layer path (['SPC'], ['SPC', 'v']) or null when closed. Draw it with menuLayer(path, ctx). */
  menu: string[] | null
}

export interface KeyRouterOptions {
  platform: Platform
  getContext: () => KeyContext
  /** Runs the action. `false` says the handler did not use the key: the router then leaves it to the browser. */
  dispatch: (action: DispatchedAction) => boolean | void
  table?: KeyTable
  /** Called when the pending sequence or the open menu layer changes. */
  onChange?: (state: RouterState) => void
}

export interface KeyRouter {
  handle(event: KeyEventLike): KeyResult
  /** Act as if `key` (a token such as 'v', 'SPC', 'Backspace') was pressed — a click on a menu item. */
  press(key: string): KeyResult
  state(): RouterState
  /** Drop the pending sequence and close the menu. */
  reset(): void
  /**
   * Drop a sequence or menu held in a mode the panel has left (focus moved into an input, the tab changed). Call
   * it whenever the mode may have changed, so a menu is not left drawn with items that no longer run.
   */
  settle(): void
}

const NAMED: Readonly<Record<string, string>> = {
  " ": "SPC",
  Spacebar: "SPC",
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
  Up: "Up",
  Down: "Down",
  Left: "Left",
  Right: "Right",
  Escape: "Esc",
  Esc: "Esc",
  Enter: "Enter",
  Tab: "Tab",
  Backspace: "Backspace",
  Delete: "Delete",
  Home: "Home",
  End: "End",
  PageUp: "PageUp",
  PageDown: "PageDown",
}

const NOT_KEYS = new Set([
  "Shift", "Control", "Alt", "AltGraph", "Meta", "OS", "Super", "Hyper", "CapsLock", "NumLock", "ScrollLock",
  "Fn", "FnLock", "Dead", "Unidentified", "Process", "Compose", "HangulMode", "HanjaMode", "KanaMode",
])

const ASCII_PRINTABLE = /^[\x20-\x7e]$/
const DIGIT_SHIFT = ")!@#$%^&*("

/**
 * The key a keydown stands for. With a Korean (or any non-Latin) input source event.key is a jamo ('ㅓ' for J) or
 * 'Process'; then the US-layout character of event.code (KeyA..KeyZ, Digit0..Digit9) is used, with Shift honoured.
 * An ASCII event.key is left alone (AZERTY, Dvorak). Anything else without such a code is returned as it was.
 */
function codeKey(event: KeyEventLike): string {
  const key = event.key
  if (!key || ASCII_PRINTABLE.test(key)) return key
  if (key.length > 1 && key !== "Process" && /^[A-Za-z]/.test(key)) return key // a named key (Enter, F1, ...)
  const m = /^(?:Key([A-Z])|Digit([0-9]))$/.exec(event.code ?? "")
  if (!m) return key
  if (m[1]) return event.shiftKey ? m[1] : m[1].toLowerCase()
  return event.shiftKey ? DIGIT_SHIFT[Number(m[2])]! : m[2]!
}

/** A keydown to its token in the table's notation, or null when it is not a key the panel handles. */
export function keyToken(event: KeyEventLike): string | null {
  const key = codeKey(event)
  if (!key || event.metaKey || NOT_KEYS.has(key)) return null
  const ctrl = event.ctrlKey ? "C-" : ""
  const alt = event.altKey ? "A-" : ""
  const named = NAMED[key]
  // Shift is part of a named key's token only where the table has one (S-Tab). Space, Enter, Esc and the
  // arrows are the same key with Shift still down from the capital typed before them (keys.rs: Shift is
  // dropped for them).
  if (named) return `${ctrl}${alt}${event.shiftKey && named === "Tab" ? "S-" : ""}${named}`
  // A printable character already carries Shift ('G', '?').
  if ([...key].length === 1) return `${ctrl}${alt}${key}`
  return `${ctrl}${alt}${event.shiftKey ? "S-" : ""}${key}`
}

/** Keys whose browser default (focus move, page scroll, back) must not run in browse and wiki (§2). */
const ALWAYS_PREVENT = new Set(["SPC", "Backspace", "/"])
const baseKey = (token: string): string => token.replace(/^(?:[CAS]-)+(?=.)/, "")

const LEADER = "SPC"

export function createKeyRouter(options: KeyRouterOptions): KeyRouter {
  const table = options.table ?? KEY_TABLE
  const platform = options.platform
  let buffer: string[] = []
  /** The mode the held sequence or menu was started in. */
  let heldIn: Mode | null = null

  const snapshot = (): RouterState =>
    buffer[0] === LEADER ? { pending: [], menu: [...buffer] } : { pending: [...buffer], menu: null }

  const setBuffer = (next: string[]): void => {
    const changed = next.length !== buffer.length || next.some((t, i) => t !== buffer[i])
    buffer = next
    if (changed) options.onChange?.(snapshot())
  }

  /** Dispatch the binding's action; `used` is false when the handler said the key was not its. */
  const run = (binding: Binding, keys: string[], ctx: KeyContext, fromMenu: boolean): { action: DispatchedAction; used: boolean } => {
    const action: DispatchedAction = { id: binding.action, keys, mode: ctx.mode, fromMenu }
    if (binding.args) action.args = binding.args
    const used = options.dispatch(action) !== false
    return { action, used }
  }

  /** A key that, inside the menu, closes it and moves (menu.rs `moves`): a motion binding, or a prefix of motions only. */
  const isMotion = (token: string, ctx: KeyContext): boolean => {
    const own = table[ctx.mode]
    const found = lookup(own, [token], platform)
    if (found.kind === "run") return found.binding.motion === true && isAvailable(found.binding, ctx)
    if (found.kind === "unknown") return false
    const under = own.filter((b) => bindingKeys(b, platform).some((k) => k.startsWith(`${token} `)))
    return under.length > 0 && under.every((b) => b.motion === true)
  }

  const feedMenu = (token: string, ctx: KeyContext): KeyResult => {
    const swallowed: KeyResult = { handled: true, preventDefault: true }
    if (token === LEADER || token === "Esc") {
      setBuffer([])
      return swallowed
    }
    if (token === "Backspace") {
      setBuffer(buffer.slice(0, -1))
      return swallowed
    }
    const menu = table.menu
    const seq = [...buffer, token]
    const found = lookup(menu, seq, platform)
    if (found.kind === "run") {
      // An item the layer carries but cannot run now is ignored — it does not leak into a motion (menu.rs review note).
      if (!isAvailable(found.binding, ctx)) return swallowed
      if (found.binding.kind !== "toggle") setBuffer([])
      return { ...swallowed, action: run(found.binding, seq, ctx, true).action }
    }
    if (found.kind === "pending") {
      if (live(menu, seq, ctx, platform)) setBuffer(seq)
      return swallowed
    }
    if (isMotion(token, ctx)) {
      setBuffer([])
      return feedDirect(token, ctx)
    }
    return swallowed
  }

  const feedDirect = (token: string, ctx: KeyContext): KeyResult => {
    const guard = (ctx.mode === "browse" || ctx.mode === "wiki") && ALWAYS_PREVENT.has(baseKey(token))
    const held = buffer.length > 0
    if (!held && token === LEADER && (ctx.mode === "browse" || ctx.mode === "wiki")) {
      if (live(table.menu, [LEADER], ctx, platform)) {
        setBuffer([LEADER])
        return { handled: true, preventDefault: true }
      }
      return { handled: false, preventDefault: guard }
    }
    const seq = [...buffer, token]
    const found = lookup(bindingsFor(ctx.mode, table), seq, platform)
    if (found.kind === "pending") {
      setBuffer(seq)
      return { handled: true, preventDefault: true }
    }
    setBuffer([])
    if (found.kind === "run" && isAvailable(found.binding, ctx)) {
      const { action, used } = run(found.binding, seq, ctx, false)
      // A handler that did not use the key (the wiki search's Tab, ↑): the browser's default runs (focus, caret).
      if (!used && !held) return { handled: false, preventDefault: guard, action }
      return { handled: true, preventDefault: true, action }
    }
    // A key that ends a held sequence without meaning is swallowed with it (keys.rs Chord::feed).
    return { handled: held, preventDefault: held || guard }
  }

  const feed = (token: string, ctx: KeyContext): KeyResult => {
    // A sequence or menu held in another mode is dropped: prompt and sorter, and also a click that switched
    // browse ↔ wiki under an open browse-only layer.
    if (buffer.length > 0 && heldIn !== ctx.mode) setBuffer([])
    heldIn = ctx.mode
    if (buffer[0] === LEADER) return feedMenu(token, ctx)
    return feedDirect(token, ctx)
  }

  const none: KeyResult = { handled: false, preventDefault: false }

  return {
    handle(event) {
      const ctx = options.getContext()
      if (event.isComposing || event.keyCode === 229) {
        // An IME at work in an editable target (or the prompt) is its own. In a non-editable one Chrome sends
        // 'Process'/a jamo: only a key event.code can name goes on ('Process' without one is in NOT_KEYS).
        if (ctx.mode === "prompt" || (event.editable ?? isEditableTarget(event.target))) return none
        if (codeKey(event) === event.key) return none
      }
      const token = keyToken(event)
      if (token === null) return none
      if (ctx.mode !== "prompt") {
        const editable = event.editable ?? isEditableTarget(event.target)
        if (editable) {
          setBuffer([])
          return none
        }
      }
      // Holding SPC, or a key inside the menu, must not flip the menu or a toggle back and forth on every repeat.
      if (event.repeat && (ctx.mode === "browse" || ctx.mode === "wiki") && (token === LEADER || buffer[0] === LEADER)) {
        return { handled: true, preventDefault: true }
      }
      return feed(token, ctx)
    },
    press(key) {
      const ctx = options.getContext()
      // A click on an item of a menu that a change of mode already dropped (the overlay stays up until it is
      // redrawn): that is not a key to run in the new mode ('r' would mark the row read there).
      if (buffer.length > 0 && heldIn !== ctx.mode) {
        setBuffer([])
        return none
      }
      return feed(key, ctx)
    },
    state: snapshot,
    reset() {
      setBuffer([])
    },
    settle() {
      if (buffer.length > 0 && heldIn !== options.getContext().mode) setBuffer([])
    },
  }
}

const TEXTLESS_INPUTS = new Set(["button", "checkbox", "color", "file", "hidden", "image", "radio", "range", "reset", "submit"])

interface ElementLike {
  tagName?: string
  type?: string
  isContentEditable?: boolean
  closest?: (selector: string) => unknown
}

/** input (text-like), textarea, select or contenteditable. Duck-typed so it runs without DOM globals. */
export function isEditableTarget(target: unknown): boolean {
  if (!target || typeof target !== "object") return false
  const el = target as ElementLike
  const tag = (el.tagName ?? "").toUpperCase()
  if (tag === "TEXTAREA" || tag === "SELECT") return true
  if (tag === "INPUT") return !TEXTLESS_INPUTS.has((el.type ?? "text").toLowerCase())
  if (el.isContentEditable) return true
  return typeof el.closest === "function" && el.closest('[contenteditable]:not([contenteditable="false"])') != null
}

export interface KeydownTarget {
  addEventListener(type: "keydown", listener: (event: KeyboardEventLike) => void): void
  removeEventListener(type: "keydown", listener: (event: KeyboardEventLike) => void): void
}

export interface KeyboardEventLike extends KeyEventLike {
  defaultPrevented?: boolean
  preventDefault(): void
}

/** Inside an open modal dialog (the shortcut help): its keys are its own, not the panel's behind it. */
function inModal(target: unknown): boolean {
  const el = target as ElementLike | null
  return typeof el?.closest === "function" && el.closest('[aria-modal="true"]:not([hidden])') != null
}

const CLICKABLE_INPUTS = new Set(["button", "checkbox", "radio", "reset", "submit", "image"])

/**
 * A control that Enter or Space activates itself (a button, a link, a checkbox or radio, a <summary>). The router
 * leaves those keys to it: running `enter` and preventDefault there would swallow the control's click.
 */
export function isActivatable(target: unknown): boolean {
  if (!target || typeof target !== "object") return false
  const el = target as ElementLike & { hasAttribute?: (name: string) => boolean; getAttribute?: (name: string) => string | null }
  const tag = (el.tagName ?? "").toUpperCase()
  if (tag === "BUTTON" || tag === "SUMMARY") return true
  if (tag === "A") return el.hasAttribute?.("href") === true
  if (tag === "INPUT") return CLICKABLE_INPUTS.has((el.type ?? "text").toLowerCase())
  const role = el.getAttribute?.("role")
  return role === "button" || role === "checkbox" || role === "radio" || role === "link" || role === "menuitem" || role === "menuitemcheckbox" || role === "tab"
}

/**
 * Space on a tab of the tab strip: the strip selects a tab as the focus moves onto it (tabs.ts), so Space has
 * nothing to click there. It is the leader: Esc out of the wiki lands on the tab, and SPC then opens the menu.
 */
function leaderOnTab(event: KeyEventLike): boolean {
  if (event.key !== " " || event.ctrlKey || event.altKey || event.metaKey) return false
  const el = event.target as { getAttribute?: (name: string) => string | null } | null
  return el?.getAttribute?.("role") === "tab"
}

export interface AttachOptions {
  /**
   * Sees every keydown first; true takes it from the router (an overlay that keeps the panel quiet while it is
   * open, even when the focus has left it). It calls preventDefault itself when it wants that.
   */
  intercept?: (event: KeyboardEventLike) => boolean
}

/**
 * Wire the router to a document's keydown. A key a component already handled (defaultPrevented: the tab strip's
 * arrows, the split resizer), pressed inside a modal dialog, or an Enter / Space on a focused control that clicks
 * on it (isActivatable) is left alone, and so is Tab there (the focus moves on) — but Space on a tab-strip tab is
 * the leader (leaderOnTab). Returns the function that unwires it.
 */
export function attachKeyRouter(doc: KeydownTarget, router: KeyRouter, options: AttachOptions = {}): () => void {
  const onKeyDown = (event: KeyboardEventLike): void => {
    if (event.defaultPrevented || inModal(event.target)) return
    if (options.intercept?.(event)) return
    if ((event.key === "Enter" || event.key === " " || event.key === "Tab") && isActivatable(event.target) && !leaderOnTab(event)) {
      const held = router.state()
      if (held.menu === null && held.pending.length === 0) return
    }
    const result = router.handle({
      key: event.key,
      ctrlKey: event.ctrlKey,
      metaKey: event.metaKey,
      altKey: event.altKey,
      shiftKey: event.shiftKey,
      isComposing: event.isComposing,
      keyCode: event.keyCode,
      code: event.code,
      repeat: event.repeat,
      editable: isEditableTarget(event.target),
      target: event.target,
      source: event,
    })
    if (result.preventDefault) event.preventDefault()
  }
  doc.addEventListener("keydown", onKeyDown)
  return () => doc.removeEventListener("keydown", onKeyDown)
}

/** 'mac' when the user agent / platform says macOS (Ctrl-w reaches the panel only there). */
export function detectPlatform(nav: { platform?: string; userAgent?: string } | undefined): Platform {
  const text = `${nav?.platform ?? ""} ${nav?.userAgent ?? ""}`
  return /Mac|iPhone|iPad/i.test(text) ? "mac" : "other"
}
