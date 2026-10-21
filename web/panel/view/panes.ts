// Which view fills each pane of the shell (contract: view/types.ts). One import and one entry per pane, on
// lines of their own, so the stage that replaces a pane's view edits only its own two lines.
import type { Panes } from "./types.ts"
import { mountList } from "./list.ts"
import { mountBoard } from "./board.ts"
import { mountDetail } from "./detail.ts"
import { mountWiki } from "./wiki.ts"

export const PANES: Panes = {
  list: mountList,
  board: mountBoard,
  detail: mountDetail,
  wiki: mountWiki,
}
