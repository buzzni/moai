/**
 * The contract between the shell (view/shell.ts) and the views it mounts into its panes.
 *
 * - The shell owns the frame: tab bar, path line, banners, add field and the split. It makes one root
 *   element per pane and calls a {@link MountPane} once for each:
 *   `mountList(root, ctx)` and `mountBoard(root, ctx)` draw into the main pane (the list side of the split),
 *   `mountDetail(root, ctx)` into the detail pane, and `mountWiki(root, ctx)` into the wiki tab's panel.
 *   view/panes.ts names which function fills each pane.
 * - After that the shell calls `render(state, index)` whenever the store changes, for the panes that show:
 *   the active tab's main pane (list or board) and, beside it, the detail; the wiki pane on the wiki tab. A
 *   pane that is not showing is not rendered, and gets a render as soon as it shows again. It is also
 *   rendered again when the main pane's width crosses a column breakpoint ({@link PaneLayout.mainWidth}).
 * - Views read the state they are given and report what the user did through {@link ViewIntents}; they keep
 *   no copy of the store's state. The root they get stays theirs: the shell never writes inside it.
 *   The main pane's root scrolls (overflow: auto) and its scroll offsets survive tab changes.
 * - Keys: the key router (panel/keys/router.ts, wired by panel/keys/wiring.ts) is the one keydown listener; views
 *   do not listen for keys themselves, so no key runs twice. The router turns a key into an action id and hands
 *   the pane that has the focus {@link PaneView.command} (the list's and board's moves, going in and out); the
 *   wiki pane takes its keys through {@link PaneView.handleKey}, where `false` means "not mine" and the key goes
 *   on (the wiki's Esc with nothing left to clear moves the focus to the tab bar).
 */
import type { IssueIndex, Path } from '../data/index.js'
import type { Copy } from '../i18n/index.js'
import type { Action, DetailPosition, Focus, State } from '../store.js'

/** What the user asked for. The shell turns each into store actions and reads (see shell.ts). */
export interface ViewIntents {
  /** Go into a place (`[]` is the root). `cursor` also moves the cursor; left out, it stays. */
  enter(path: Path, cursor?: string | null): void
  /** Put the cursor on a row and show its detail (reads `show <id>`). */
  choose(id: string): void
  /** Put the cursor on a row without reading its detail (a keyboard move, a hover preview). */
  point(id: string | null): void
  /** Close the detail of the row shown; the pane stays and says nothing is chosen. */
  closeDetail(): void
  /** Show or hide the detail pane (`SPC v d`). */
  setDetailVisible(visible: boolean): void
  /** Unfold or fold a folder in the tree. */
  toggleExpanded(path: Path): void
  /** Read the list again now. */
  refresh(): void
  /** Any other pure state change (a sort header click, a column toggle). Prefer a named intent when there is one. */
  dispatch(action: Action): void
}

/** The layout the panes are drawn in right now. */
export interface PaneLayout {
  /** The panel's width in px (the frame's). */
  width: number
  /**
   * The main pane's width in px: the whole width, less the detail's share when the detail stands on the left.
   * Pass it to `visibleColumns(mainWidth, enabled)` (view/responsive.ts) to drop columns that no longer fit.
   */
  mainWidth: number
  /** Where the detail stands now — a panel under 640px forces the bottom whatever was chosen. */
  position: DetailPosition
  detailVisible: boolean
}

/**
 * Where the wiki tab reads from: `moai wiki ls --json` and `moai wiki show <slug> --json`, parsed or not
 * (view/wiki.ts parseWikiLs / parseWikiShow read either). Without one the wiki tab says it cannot read the wiki.
 */
export interface WikiSource {
  list(): Promise<unknown>
  page(slug: string): Promise<unknown>
}

export interface ViewContext {
  document: Document
  copy: Copy
  /** BCP 47 tag of the host document, for dates and numbers. */
  language: string
  intents: ViewIntents
  layout(): PaneLayout
  /** Move the keyboard focus into a pane: its view's `focus()` when it has one, else the pane's root. */
  focusPane(area: Focus): void
  /** The wiki's reads; absent until the data layer gives them (main.ts). */
  wiki?: WikiSource
}

export interface PaneView {
  render(state: State, index: IssueIndex | null): void
  /** Put the keyboard focus where the view wants it (the cursor row, the detail's first control). */
  focus?(): void
  /**
   * A key for this pane, from the key router (panel/keys/). Returns true when the view used it (and called
   * preventDefault), false when the key is not its own and should go on — e.g. the wiki's Esc with no link
   * selected and no search left to clear, which the key wiring (keys/wiring.ts) turns into a move to the tab bar.
   */
  handleKey?(event: KeyboardEvent): boolean
  /**
   * A key-table action for this pane (keys/table.ts ids: `move.down`, `enter`, `expand`, `focus.next`, …), from
   * the key router. Returns true when the pane did it, false when the action is not one it knows.
   */
  command?(action: string, args?: Readonly<Record<string, string | number>>): boolean
  /** `SPC v r`: the body as raw markdown or rendered (the wiki's page, the detail's body; the router calls it). */
  toggleRaw?(): void
  /** The body is shown as raw markdown now (the `SPC v r` menu item's state). */
  raw?(): boolean
  /** The pane's tab was left: save what should come back with it into the store (the wiki's page and trail). */
  leave?(): void
  destroy?(): void
}

/** `mountList`, `mountBoard`, `mountDetail` and `mountWiki` all have this shape. */
export type MountPane = (root: HTMLElement, ctx: ViewContext) => PaneView

/** The view filling each pane. Without `wiki` the wiki tab shows a placeholder. */
export interface Panes {
  list: MountPane
  board: MountPane
  detail: MountPane
  wiki?: MountPane
}
