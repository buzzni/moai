/**
 * The page's words are not here. They live in moai's own bundles, `i18n/{en,ko,ja,zh,es}.json`, under the `page.`
 * prefix, next to the CLI's keys (i18n/README.md). This file is the lookup: it nests the flat keys into the groups
 * below (`page.shell.tabs.board` -> `copy.shell.tabs.board`; `.0`, `.1` are array items) and falls back to English
 * key by key, as moai does.
 *
 * The words are bundled, not served by `/api/boot`: the page has to speak when the server cannot be reached (the
 * boot is the call that failed), and several modules read them at import time. web/build.mjs filters each JSON file
 * to the `page.` keys (web/i18n-bundles.mjs), so the CLI's own words stay out of the bundle.
 *
 * A word that carries a value holds a `{name}` place; `fill(copy.x, { name })` puts the value there.
 */
import type { VersionCopy } from '../data/about.js'
import type { SortKey } from '../data/query.js'
import type { CellField, HelpGroupId, KeyLabel, LayerId, SortField, StateId } from '../keys/table.js'
import en from 'moai-i18n:en'
import es from 'moai-i18n:es'
import ja from 'moai-i18n:ja'
import ko from 'moai-i18n:ko'
import zh from 'moai-i18n:zh'

export type Language = 'en' | 'ko' | 'ja' | 'zh' | 'es'

/**
 * Things to say to an agent session, shown one at a time under the header's Version line and all together, by kind, in
 * the tips page (view/tips.ts, se-irhr). The field test gave the first of them; the rest fill the kinds out.
 */
export interface TipsCopy {
  /** The header line's label stays English, as the header's others do; this is the page's title. */
  title: string
  intro: string
  /** The header line's accessible name: it opens the page. */
  openAll: string
  copy: string
  copied: string
  close: string
  groups: ReadonlyArray<{ title: string; tips: readonly string[] }>
}

export interface ShellCopy {
  /** Accessible name of the tab strip. */
  tabsLabel: string
  tabs: { board: string; list: string; wiki: string; gantt: string }
  /** The position toggle's name says what pressing does; under 640px it says why it is off. */
  detailToRight: string
  detailToTop: string
  detailToLeft: string
  detailToBottom: string
  detailForcedBottom: string
  showDetail: string
  hideDetail: string
  /** The theme button: the state it is in, then what pressing it does (the accessible name says both). */
  theme: { system: string; light: string; dark: string; toSystem: string; toLight: string; toDark: string }
  resizer: string
  /** The refresh-time button before the first read finished. */
  refreshNow: string
  /** Accessible name of the path line. */
  pathLabel: string
  /** The root of the path, as the TUI writes it. */
  rootSegment: string
  /** `{names} hidden` — the view toggles that hide rows. */
  hidden: string
  deferred: string
  backlog: string
  /** `sort {by}` — only when the order is not the default. */
  sorted: string
  sortFields: Record<SortKey, string>
  /** After the order's words when the whole list is turned around. */
  reversed: string
  /** After a search or filter badge: how to take it off. */
  clearHint: string
  wikiPlaceholder: string
  /** The wiki tab with no wiki reads to call (the data layer has not given them). */
  wikiUnavailable: string
  /** `wiki ls` failed; the reason follows. */
  wikiListFailed: string
  /** The archive read failed (`SPC v o`); why follows. The board's rows stand. */
  archiveFailed: string
  /** The header's Version line (view/brand.ts). */
  version: VersionCopy
  /** The header's tips and the tips page (view/tips.ts). */
  tips: TipsCopy
}

export interface ListCopy {
  /** Accessible name of the list. */
  label: string
  /** The `..` line's name: one level up. */
  upLabel: string
  /** The ▸ button in front of a folder. */
  expand: string
  collapse: string
  /** Column heads of the name line. */
  heads: { id: string; priority: string; status: string; title: string; counts: string; tags: string; epic: string; assignee: string; created: string; updated: string }
  /** A column head's name when clicking it sorts by that column. */
  sortBy: string
  /** What the view toggles hide under this place. */
  hiddenCount: string
  /** Lifts the status, deferred and backlog hiding (`SPC v a`); archived rows stay hidden, and the words say so. */
  showAll: string
  /** After the title of a row the view hides that a search shows anyway. */
  hiddenMark: string
}

export interface BoardCopy {
  /** Accessible name of the board. */
  label: string
  /** Accessible name of a status column: its name and card count. */
  columnLabelOne: string
  /** The same, for any count but one (`{name}`, `{count}`). */
  columnLabelMany: string
  /** An empty column. */
  noCards: string
}

export interface GanttCopy {
  /** Accessible name of the gantt. */
  label: string
  /** No line to draw. */
  empty: string
  /** The grid's head line. */
  heads: { id: string; title: string; progress: string; assignee: string }
  zoomLabel: string
  /** The four steps' names. */
  zoom: { hour: string; day: string; week: string; month: string }
  today: string
  fit: string
  /** Accessible name of the handle between the grid and the dates. */
  resizer: string
  /** Sunday first. */
  weekdays: readonly string[]
  /** The date header's lines (`{year}`, `{month}`, `{day}`, `{weekday}`, `{week}`). */
  axis: { year: string; yearMonth: string; month: string; day: string; hourDay: string; week: string }
  /** The bubble over a bar. */
  tip: {
    planned: string
    plannedMembers: string
    actual: string
    now: string
    unknown: string
    notStarted: string
    overdue: string
    /** One day past the deadline (`{n}` is 1). */
    overdueDay: string
    /** `{n}` days past the deadline. */
    overdueDays: string
    progress: string
    blockedBy: string
    deferred: string
    deferredNote: string
    assignee: string
  }
  /** The legend under the dates; each entry is drawn with the class of the bar it names. */
  legend: { planned: string; members: string; deferred: string; actual: string; overdue: string; due: string; today: string; clipped: string; blocker: string }
  /** Moving a bar (moai-d86e.3ud): `{id}` the row, `{cmd}` the `moai edit` line that puts its days back. */
  move: { moved: string; undo: string; members: string; none: string }
}

export interface DetailCopy {
  /** Accessible name of the detail pane. */
  label: string
  /** The pane is shown but nothing is chosen. */
  none: string
  /** Labels of the fields under the head line. */
  assignee: string
  created: string
  updated: string
  started: string
  ended: string
  due: string
  epic: string
  milestone: string
  blockedBy: string
  blocks: string
  deferred: string
  /** A row put off because the epic, milestone or parent it stands under is (`shelved_by`). */
  deferredUnder: string
  /** A blocker's state: finished, still open, put off, or not in the index. */
  blockerDone: string
  blockerOpen: string
  blockerDeferred: string
  blockerMissing: string
  /** Roll-up of a group: `{done}/{work} done · {percent}%`. */
  rollup: string
  rollupText: string
  /** The same with the share done (`{done}`, `{work}`, `{percent}`). */
  rollupTextPercent: string
  body: string
  commits: string
  /** The wiki pages about the row (`show <id> --json`'s `wiki`): the section and its three groups, and a lesson's mark. */
  wiki: string
  wikiNamed: string
  wikiRelated: string
  wikiLessons: string
  wikiLesson: string
  journal: string
  noJournal: string
  work: string
  subtasks: string
  kinds: Record<string, string>
  /** The head line's pN. */
  priority: string
  /** Words of a journal line. */
  created_entry: string
  /** The detail's read failed; the reason follows from the refusal words. */
  problem: string
  close: string
}

/** Why the list would hide a row that a search shows (view/list-model.ts viewHides): the view toggle that hides it. */
export type HiddenReason = 'column' | 'deferred' | 'backlog' | 'archived'

export interface ToolbarCopy {
  search: string
  searchPlaceholder: string
  filter: string
  filterPlaceholder: string
  removeChip: string
  /** Accessible name of the filter's suggestion list. */
  hints: string
  sort: string
  sortEdit: string
  reversed: string
  replace: string
  append: string
  flip: string
  sortKeys: Record<SortKey, string>
  /** `N hidden` — rows the search shows that the view hides. */
  hidden: string
  hiddenReasons: Record<HiddenReason, string>
  /** The sort editor's note on `D`: the default it saves lasts for this session (keys/wiring.ts hides it where it persists). */
  sortDefaultSessionNote: string
}

export interface KeysCopy {
  /** Shown while the panel's frame has no keyboard focus (the host does not give it one). */
  focusHint: string
  /** `D` in the sort editor: the default order is saved where it outlives the panel. */
  sortDefaultSaved: string
  /** `D` in the sort editor while no settings storage keeps it: saved for this session only. */
  sortDefaultSavedSession: string
  /** `r`, `SPC m a`, `SPC m g` in a page given no read-mark writes (keys/wiring.ts `read`). */
  readUnavailable: string
  /** `SPC m g` with no milestone or epic under the cursor or around the list. */
  readNoGroup: string
  /** The zone picker (`SPC o t`, view/zones.ts), in the TUI's words where it has them (`tui.tz.*`). */
  zone: ZoneCopy
}

export interface ZoneCopy {
  /** `tui.tz.title`. */
  title: string
  /** The filter box's placeholder. */
  filter: string
  /** Beside the name in use (`tui.tz.now`). */
  now: string
  /** The first line, which clears the page's choice: the browser's zone follows (`{zone}` is its name). */
  browser: string
  /** The same line when the TUI picked a zone (`/api/boot`'s `timezone`): that one follows. */
  tui: string
  /** Nothing on the list holds what was typed (`tui.tz.no_match`). */
  noMatch: string
  /** The zone in use is not on the list (`tui.tz.unlisted`): Enter alone keeps it. */
  unlisted: string
}

/**
 * The key table names each binding by a label id; these are the words for them. The English follows moai's own
 * `tui.*` words where the panel shows the same thing as the TUI: the state labels (`tui.state.*`, without the
 * TUI's `●` sort mark) and the root and layer names (`tui.menu.root`, `tui.group.*`), except `SPC g`, which the
 * panel calls Go (`tui.group.screen` is "screen") and `SPC w`, which the TUI has not.
 */
export interface KeyTableCopy {
  labels: Record<KeyLabel, string>
  /** The SPC menu's root title (`tui.menu.root`). */
  root: string
  layers: Record<LayerId, string>
  groups: Record<HelpGroupId, string>
  /** A menu item's state, brackets included (`tui.state.*`). */
  states: Record<StateId, string>
  sortFields: Record<SortField, string>
  cellFields: Record<CellField, string>
  /** `SPC s P` and the like: the field put at the end of the order. */
  sortAdd: string
  /** The help overlay's longer words for the same item (the menu's cell is narrow). */
  sortAddHelp: string
  /** `SPC v N` when the board has fewer status columns named than N. */
  column: string
  /** The help row standing for `SPC v 1~9`. */
  columns: string
  /** The help overlay's head. */
  helpTitle: string
  close: string
}

export interface WikiCopy {
  /** The reads' answers could not be used; the parse error or the shape follows. */
  badJson: string
  listShape: string
  listNoPages: string
  folderFailed: string
  pageFailed: string
  pageShape: string
  /** Warning chips on a page (`tui.wiki.mark_*`) and their longer words. */
  conflict: string
  conflictDetail: string
  unread: string
  large: string
  largeDetail: string
  searchPlaceholder: string
  searchLabel: string
  pagesLabel: string
  back: string
  raw: string
  rawTitle: string
  bodyLabel: string
  resizer: string
  loadingList: string
  noPages: string
  /** Under "no pages": where to put them. */
  noPagesHint: string
  noMatch: string
  noMatchHint: string
  backlinksLabel: string
  backlinks: string
  noBacklinks: string
  backlinksPartial: string
  choosePage: string
  choosePageHint: string
  loadingPage: string
  pageConflict: string
  bodyFailed: string
  renderFailed: string
  /** `#anchor` in raw view, where headings are not targets. */
  rawNoAnchor: string
  noAnchor: string
  noLinks: string
  /** `tui.wiki.outside`. */
  outside: string
  loadFailed: string
  nothingBack: string
}

export interface Copy {
  hint: string
  refreshedAt: string
  empty: string
  up: string
  root: string
  noMilestone: string
  lost: string
  loading: string
  /** A read is out to `moai web` (the activity line). */
  running: string
  cancel: string
  cancelled: string
  stats: Record<string, string>
  /** `moai web` could not be reached (client.ts `not-started`). */
  unreachable: string
  /** Reads keep failing against a server that answers: they now come every 10 s, not every 2 s (data/refresh.ts). */
  backingOff: string
  moaiError: string
  failed: string
  unreadable: string
  /** Before the lines moai said on stderr beside an answer (client.ts Outcome `notes`). */
  said: string
  shell: ShellCopy
  list: ListCopy
  board: BoardCopy
  gantt: GanttCopy
  detail: DetailCopy
  toolbar: ToolbarCopy
  keys: KeysCopy
  table: KeyTableCopy
  wiki: WikiCopy
}

// ── the lookup ─────────────────────────────────────────────────────────────────────────────────────────

const PREFIX = 'page.'

const BUNDLES: Record<Language, Readonly<Record<string, string>>> = { en, ko, ja, zh, es }

type Node = { [key: string]: string | Node }

/** `{name}` places filled from `vars`; a place without a value stays as written. */
export function fill(text: string, vars: Record<string, string | number>): string {
  return text.replace(/\{(\w+)\}/g, (place, name: string) => (Object.hasOwn(vars, name) ? String(vars[name]) : place))
}

/** Objects whose keys are all 0..n-1 become arrays (the tips lists); everything else keeps its keys. */
function arrayed(node: string | Node): unknown {
  if (typeof node === 'string') return node
  const keys = Object.keys(node)
  const out: Record<string, unknown> = {}
  for (const key of keys) out[key] = arrayed(node[key]!)
  if (keys.length > 0 && keys.every((key, at) => key === String(at))) return keys.map((key) => out[key])
  return out
}

/** Groups whose own names hold dots (the key table's label ids: `read` and `read.all`): the rest of the key is one name. */
const DOTTED_GROUPS = ['table.labels.']

function split(path: string): string[] {
  const group = DOTTED_GROUPS.find((head) => path.startsWith(head))
  return group ? [...group.slice(0, -1).split('.'), path.slice(group.length)] : path.split('.')
}

function build(language: Language): Copy {
  const own = BUNDLES[language]
  const root: Node = {}
  for (const [key, english] of Object.entries(BUNDLES.en)) {
    if (!key.startsWith(PREFIX)) continue
    const path = split(key.slice(PREFIX.length))
    let at = root
    for (const step of path.slice(0, -1)) at = (at[step] ??= {}) as Node
    at[path[path.length - 1]!] = own[key] ?? english
  }
  return arrayed(root) as Copy
}

const LANGUAGES: readonly Language[] = ['en', 'ko', 'ja', 'zh', 'es']

/** Every language, whole: a key a language lacks reads in English. */
export const COPY = Object.fromEntries(LANGUAGES.map((language) => [language, build(language)])) as Record<Language, Copy>

export const TABLE = Object.fromEntries(LANGUAGES.map((language) => [language, COPY[language].table])) as Record<Language, KeyTableCopy>

/** The language of a tag (`/api/boot`'s `lang`: `ko`, `ko-KR`), or null for one with no `page.` key of its own (es today). */
function carried(language: string | undefined): Language | null {
  const base = (language || 'en').split('-')[0]!.toLowerCase()
  if (!Object.hasOwn(BUNDLES, base)) return null
  return base === 'en' || Object.keys(BUNDLES[base as Language]).some((key) => key.startsWith(PREFIX)) ? (base as Language) : null
}

/** Whether the page has words of its own in this language; one it has not is drawn, and named, in English. */
export function speaks(language: string | undefined): boolean {
  return carried(language) !== null
}

/** The words for a language tag (`/api/boot`'s `lang`); a language the page does not carry is English. */
export function copyFor(language: string | undefined): Copy {
  return COPY[carried(language) ?? 'en']
}
