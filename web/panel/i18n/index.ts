/**
 * The panel's words in English, Korean, Japanese and Chinese. The top level is carried over from the 1.x
 * panel; newer words live in one group per area (`shell`, `list`, `board`, `detail`, `toolbar`, `keys`, `table`,
 * `wiki`), each with its own interface and its four languages side by side below, so the stages that build those
 * areas add their words inside their own group without touching another's lines.
 */
import type { RefusalCode } from '../data/client.js'
import type { Feature } from '../data/compat.js'
import type { GrepScope, SortKey } from '../data/query.js'
import type { CellField, HelpGroupId, KeyLabel, LayerId, SortField, StateId } from '../keys/table.js'

export type Language = 'en' | 'ko' | 'ja' | 'zh'

// ── shell: tab bar, toggles, path line, banners (E2 se-qwkz.2v9) ────────────────────────────────────

/** The compatibility banner (data/compat.ts report): one sentence per thing to fix on that machine (se-93p3). */
export interface CompatCopy {
  /** `moai` is not installed where the daemon runs. */
  toolMissing: string
  /** An older moai: the features the panel uses that are on a fallback follow (one at least). */
  oldMoai: (features: string[]) => string
  /** The daemon's catalog lacks `missing` of the panel's profiles; the features they serve follow. */
  catalog: (missing: number, features: string[]) => string
  features: Record<Feature, string>
}

const COMPAT_EN: CompatCopy = {
  toolMissing: 'Moai is not installed on that machine. Install moai under Machine tools in Settings → Extensions.',
  oldMoai: (features) => `That machine's moai is older than this panel expects${features.length ? ` (on a fallback: ${features.join(', ')})` : ''}. Upgrade moai.`,
  catalog: (missing, features) => `That machine's Happy daemon catalog lacks ${missing === 1 ? '1 Moai profile' : `${missing} Moai profiles`} this panel uses${features.length ? ` (${features.join(', ')})` : ''}. Update the daemon catalog.`,
  features: {
    'all-rows': 'the whole list', backlog: 'the backlog', archive: 'archived rows', wiki: 'the wiki', 'read-marks': 'read marks',
    'status-order': 'the column order', tallies: 'milestone and epic counts',
  },
}

const COMPAT_KO: CompatCopy = {
  toolMissing: '그 머신에 Moai가 설치되어 있지 않습니다. 설정 → 확장의 머신 도구에서 moai를 설치하세요.',
  oldMoai: (features) => `그 머신의 moai가 이 패널이 기대하는 버전보다 오래되었습니다${features.length ? `(대체 방식으로 동작: ${features.join(', ')})` : ''}. moai를 업그레이드하세요.`,
  catalog: (missing, features) => `그 머신의 Happy 데몬 카탈로그에 이 패널이 쓰는 Moai 프로필 ${missing}개가 없습니다${features.length ? `(${features.join(', ')})` : ''}. 데몬 카탈로그를 업데이트하세요.`,
  features: {
    'all-rows': '전체 목록', backlog: '백로그', archive: '아카이브된 항목', wiki: '위키', 'read-marks': '읽음 표시',
    'status-order': '칸 순서', tallies: '마일스톤·에픽 개수',
  },
}

const COMPAT_JA: CompatCopy = {
  toolMissing: 'そのマシンに Moai がインストールされていません。設定 → 拡張機能のマシンツールから moai をインストールしてください。',
  oldMoai: (features) => `そのマシンの moai は、このパネルが想定するバージョンより古いです${features.length ? `（代わりの方法で動作中: ${features.join('、')}）` : ''}。moai をアップグレードしてください。`,
  catalog: (missing, features) => `そのマシンの Happy デーモンのカタログに、このパネルが使う Moai プロファイルが ${missing} 個ありません${features.length ? `（${features.join('、')}）` : ''}。デーモンのカタログを更新してください。`,
  features: {
    'all-rows': '一覧全体', backlog: 'バックログ', archive: 'アーカイブ済みの項目', wiki: 'Wiki', 'read-marks': '既読マーク',
    'status-order': '列の順序', tallies: 'マイルストーンとエピックの件数',
  },
}

const COMPAT_ZH: CompatCopy = {
  toolMissing: '该机器上未安装 Moai。请在「设置 → 扩展」的机器工具中安装 moai。',
  oldMoai: (features) => `该机器上的 moai 比此面板预期的版本旧${features.length ? `（正以替代方式运行：${features.join('、')}）` : ''}。请升级 moai。`,
  catalog: (missing, features) => `该机器的 Happy 守护进程目录缺少此面板使用的 ${missing} 个 Moai 配置${features.length ? `（${features.join('、')}）` : ''}。请更新守护进程目录。`,
  features: {
    'all-rows': '完整列表', backlog: '待办池', archive: '已归档的条目', wiki: 'Wiki', 'read-marks': '已读标记',
    'status-order': '列的顺序', tallies: '里程碑和史诗的计数',
  },
}

export interface ShellCopy {
  /** Accessible name of the tab strip. */
  tabsLabel: string
  tabs: { board: string; list: string; wiki: string }
  /** The position toggle's name says what pressing does; under 640px it says why it is off. */
  detailToLeft: string
  detailToBottom: string
  detailForcedBottom: string
  showDetail: string
  hideDetail: string
  resizer: string
  /** The refresh-time button before the first read finished. */
  refreshNow: string
  /** Accessible name of the path line. */
  pathLabel: string
  /** The root of the path, as the TUI writes it. */
  rootSegment: string
  /** `{names} hidden` — the view toggles that hide rows. */
  hidden: (names: string[]) => string
  deferred: string
  backlog: string
  /** `sort {by}` — only when the order is not the default. */
  sorted: (by: string) => string
  sortFields: Record<SortKey, string>
  /** After the order's words when the whole list is turned around. */
  reversed: string
  /** After a search or filter badge: how to take it off. */
  clearHint: string
  wikiPlaceholder: string
  /** The wiki tab with no wiki reads to call (the data layer has not given them). */
  wikiUnavailable: string
  /** `wiki ls` failed; the reason follows. */
  wikiListFailed: (reason: string) => string
  /** `show --all` was cut at the output limit: the list stands on prime's rows and the backlog (source.ts loadAllRows). */
  truncated: string
  /** The archive read failed (`SPC v o`); why follows. The board's rows stand. */
  archiveFailed: string
  /** The list stands on prime's rows because `show --all` cannot be read there; says what that leaves out (se-24ey.s6v). */
  primeOnly: string
  /** The backlog read beside prime's rows was declined or failed; why follows. The rows stand without the backlog. */
  backlogFailed: string
  compat: CompatCopy
}

const SHELL: Record<Language, ShellCopy> = {
  en: {
    tabsLabel: 'Moai views',
    tabs: { board: 'Board', list: 'List', wiki: 'Wiki' },
    detailToLeft: 'Move details to the left',
    detailToBottom: 'Move details to the bottom',
    detailForcedBottom: 'Details stay at the bottom while the panel is narrower than 640px',
    showDetail: 'Show details',
    hideDetail: 'Hide details',
    resizer: 'Resize details',
    refreshNow: 'Refresh',
    pathLabel: 'Current place',
    rootSegment: '/',
    hidden: (names) => `${names.join(' · ')} hidden`,
    deferred: 'deferred',
    backlog: 'backlog',
    sorted: (by) => `sort ${by}`,
    sortFields: { priority: 'priority', created: 'created', updated: 'updated', status: 'status', assignee: 'assignee', title: 'title', id: 'id' },
    reversed: '(reversed)',
    clearHint: 'Esc clears it',
    wikiPlaceholder: 'The wiki is not here yet.',
    wikiUnavailable: 'The wiki cannot be read here yet.',
    wikiListFailed: (reason) => `The wiki pages could not be read: ${reason}`,
    truncated: 'The whole list was too large to read, so only the tasks held and ready to pick up are shown, with the backlog when it could be read.',
    archiveFailed: 'Archived rows could not be read:',
    primeOnly: 'The whole list cannot be read on this machine, so only the tasks held and ready to pick up are shown. Done, deferred and the other to-do tasks are left out, and the backlog shows only when it could be read.',
    backlogFailed: 'The backlog could not be read:',
    compat: COMPAT_EN,
  },
  ko: {
    tabsLabel: 'Moai 보기',
    tabs: { board: '칸반', list: '목록', wiki: '위키' },
    detailToLeft: '상세를 왼쪽으로',
    detailToBottom: '상세를 아래로',
    detailForcedBottom: '패널 폭이 640px보다 좁아서 상세는 아래에 둡니다',
    showDetail: '상세 보이기',
    hideDetail: '상세 숨기기',
    resizer: '상세 크기 조절',
    refreshNow: '새로고침',
    pathLabel: '현재 경로',
    rootSegment: '/',
    hidden: (names) => `${names.join(' · ')} 숨김`,
    deferred: '미룸',
    backlog: '백로그',
    sorted: (by) => `정렬 ${by}`,
    sortFields: { priority: '우선순위', created: '만든 날', updated: '고친 날', status: '칸', assignee: '담당', title: '제목', id: 'ID' },
    reversed: '(뒤집음)',
    clearHint: 'Esc로 해제',
    wikiPlaceholder: '위키는 아직 준비 중입니다.',
    wikiUnavailable: '아직 여기서는 위키를 읽을 수 없습니다.',
    wikiListFailed: (reason) => `위키 페이지 목록을 읽지 못했습니다: ${reason}`,
    truncated: '전체 목록이 너무 커서 다 읽지 못했습니다. 잡고 있는 일과 바로 잡을 수 있는 일만 보입니다. 백로그는 읽은 경우에만 함께 보입니다.',
    archiveFailed: '아카이브된 항목을 읽지 못했습니다:',
    primeOnly: '이 환경에서는 전체 목록을 읽을 수 없어 잡고 있는 일과 바로 잡을 수 있는 일만 보입니다. 완료·미룬 일과 나머지 할 일은 보이지 않고, 백로그는 읽은 경우에만 보입니다.',
    backlogFailed: '백로그를 읽지 못했습니다:',
    compat: COMPAT_KO,
  },
  ja: {
    tabsLabel: 'Moai ビュー',
    tabs: { board: 'カンバン', list: '一覧', wiki: 'Wiki' },
    detailToLeft: '詳細を左へ',
    detailToBottom: '詳細を下へ',
    detailForcedBottom: 'パネルの幅が 640px 未満のため、詳細は下に置きます',
    showDetail: '詳細を表示',
    hideDetail: '詳細を隠す',
    resizer: '詳細のサイズを変更',
    refreshNow: '再読み込み',
    pathLabel: '現在の場所',
    rootSegment: '/',
    hidden: (names) => `${names.join(' · ')} 非表示`,
    deferred: '保留',
    backlog: 'バックログ',
    sorted: (by) => `並べ替え ${by}`,
    sortFields: { priority: '優先度', created: '作成', updated: '更新', status: '列', assignee: '担当', title: 'タイトル', id: 'ID' },
    reversed: '(逆順)',
    clearHint: 'Esc で解除',
    wikiPlaceholder: 'Wiki はまだ準備中です。',
    wikiUnavailable: 'ここではまだ Wiki を読めません。',
    wikiListFailed: (reason) => `Wiki のページ一覧を読めませんでした: ${reason}`,
    truncated: '一覧全体が大きすぎて読み切れませんでした。手元のタスクとすぐ着手できるタスクだけを表示しています。バックログは読めた場合にだけ表示します。',
    archiveFailed: 'アーカイブ済みの項目を読めませんでした:',
    primeOnly: 'この環境では一覧全体を読めないため、手元のタスクとすぐ着手できるタスクだけを表示しています。完了・保留のタスクとその他の未着手タスクは表示されず、バックログは読めた場合にだけ表示します。',
    backlogFailed: 'バックログを読めませんでした:',
    compat: COMPAT_JA,
  },
  zh: {
    tabsLabel: 'Moai 视图',
    tabs: { board: '看板', list: '列表', wiki: 'Wiki' },
    detailToLeft: '将详情移到左侧',
    detailToBottom: '将详情移到底部',
    detailForcedBottom: '面板宽度小于 640px，详情固定在底部',
    showDetail: '显示详情',
    hideDetail: '隐藏详情',
    resizer: '调整详情大小',
    refreshNow: '刷新',
    pathLabel: '当前位置',
    rootSegment: '/',
    hidden: (names) => `已隐藏 ${names.join(' · ')}`,
    deferred: '推迟',
    backlog: '待办池',
    sorted: (by) => `排序 ${by}`,
    sortFields: { priority: '优先级', created: '创建', updated: '更新', status: '列', assignee: '负责人', title: '标题', id: 'ID' },
    reversed: '(反向)',
    clearHint: '按 Esc 清除',
    wikiPlaceholder: 'Wiki 尚未就绪。',
    wikiUnavailable: '这里暂时无法读取 Wiki。',
    wikiListFailed: (reason) => `无法读取 Wiki 页面列表: ${reason}`,
    truncated: '完整列表太大，未能全部读取，因此只显示已领取和可以开始的任务；待办池仅在读取成功时一并显示。',
    archiveFailed: '无法读取已归档的条目:',
    primeOnly: '此环境无法读取完整列表，因此只显示已领取和可以开始的任务。已完成、已推迟的任务和其他待办任务不显示，待办池仅在读取成功时显示。',
    backlogFailed: '无法读取待办池:',
    compat: COMPAT_ZH,
  },
}

// ── list: the tree list (E3 se-c10x) ─────────────────────────────────────────────────────────────────

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
  sortBy: (column: string) => string
  /** What the view toggles hide under this place. */
  hiddenCount: (n: number) => string
  /** Lifts the status, deferred and backlog hiding (`SPC v a`); archived rows stay hidden, and the words say so. */
  showAll: string
  /** After the title of a row the view hides that a search shows anyway. */
  hiddenMark: string
}

const LIST: Record<Language, ListCopy> = {
  en: {
    label: 'Tasks',
    upLabel: 'Up one level',
    expand: 'Expand',
    collapse: 'Collapse',
    heads: { id: 'ID', priority: 'P', status: 'Status', title: 'Title', counts: 'Done', tags: 'Tags', epic: 'Epic', assignee: 'Assignee', created: 'Created', updated: 'Updated' },
    sortBy: (column) => `Sort by ${column}`,
    hiddenCount: (n) => `${n} hidden`,
    showAll: 'Show all but archived',
    hiddenMark: 'hidden',
  },
  ko: {
    label: '할 일 목록',
    upLabel: '한 층 위로',
    expand: '펼치기',
    collapse: '접기',
    heads: { id: 'ID', priority: 'P', status: '상태', title: '제목', counts: '완료', tags: '태그', epic: '에픽', assignee: '담당', created: '만든 날', updated: '고친 날' },
    sortBy: (column) => `${column}(으)로 정렬`,
    hiddenCount: (n) => `${n}개 숨김`,
    showAll: '모두 보이기 (아카이브 제외)',
    hiddenMark: '숨김',
  },
  ja: {
    label: 'タスク一覧',
    upLabel: '一つ上の階層へ',
    expand: '展開',
    collapse: '折りたたむ',
    heads: { id: 'ID', priority: 'P', status: '状態', title: 'タイトル', counts: '完了', tags: 'タグ', epic: 'エピック', assignee: '担当', created: '作成', updated: '更新' },
    sortBy: (column) => `${column}で並べ替え`,
    hiddenCount: (n) => `${n} 件を非表示`,
    showAll: 'すべて表示 (アーカイブは除く)',
    hiddenMark: '非表示',
  },
  zh: {
    label: '任务列表',
    upLabel: '返回上一层',
    expand: '展开',
    collapse: '折叠',
    heads: { id: 'ID', priority: 'P', status: '状态', title: '标题', counts: '完成', tags: '标签', epic: '史诗', assignee: '负责人', created: '创建', updated: '更新' },
    sortBy: (column) => `按${column}排序`,
    hiddenCount: (n) => `已隐藏 ${n} 项`,
    showAll: '全部显示 (归档除外)',
    hiddenMark: '已隐藏',
  },
}

// ── board: the kanban (E3 se-c10x) ───────────────────────────────────────────────────────────────────

export interface BoardCopy {
  /** Accessible name of the board. */
  label: string
  /** Accessible name of a status column: its name and card count. */
  columnLabel: (name: string, count: number) => string
  /** An empty column. */
  noCards: string
}

const BOARD: Record<Language, BoardCopy> = {
  en: {
    label: 'Board',
    columnLabel: (name, count) => `${name}, ${count} ${count === 1 ? 'card' : 'cards'}`,
    noCards: 'No cards',
  },
  ko: {
    label: '칸반',
    columnLabel: (name, count) => `${name}, 카드 ${count}장`,
    noCards: '카드 없음',
  },
  ja: {
    label: 'カンバン',
    columnLabel: (name, count) => `${name}、カード ${count} 枚`,
    noCards: 'カードなし',
  },
  zh: {
    label: '看板',
    columnLabel: (name, count) => `${name}，${count} 张卡片`,
    noCards: '没有卡片',
  },
}

// ── detail: the detail pane (E3 se-c10x) ─────────────────────────────────────────────────────────────

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
  deferredUnder: (id: string) => string
  /** A blocker's state: finished, still open, put off, or not in the index. */
  blockerDone: string
  blockerOpen: string
  blockerDeferred: string
  blockerMissing: string
  /** Roll-up of a group: `{done}/{work} done · {percent}%`. */
  rollup: string
  rollupText: (done: number, work: number, percent: number | undefined) => string
  body: string
  commits: string
  journal: string
  noJournal: string
  work: string
  subtasks: string
  kinds: Record<string, string>
  /** The head line's pN. */
  priority: (n: number) => string
  /** Words of a journal line. */
  created_entry: string
  /** The detail's read failed; the reason follows from the refusal words. */
  problem: string
  close: string
}

const DETAIL: Record<Language, DetailCopy> = {
  en: {
    label: 'Details',
    none: 'Choose a task to see its details.',
    assignee: 'Assignee', created: 'Created', updated: 'Updated', started: 'Started', ended: 'Ended', due: 'Due', epic: 'Epic', milestone: 'Milestone',
    blockedBy: 'Blocked by', blocks: 'Blocks', deferred: 'Deferred',
    deferredUnder: (id) => `Deferred — under ${id}`,
    blockerDone: 'done', blockerOpen: 'open', blockerDeferred: 'deferred', blockerMissing: 'not found',
    rollup: 'Progress', rollupText: (done, work, percent) => `${done}/${work} done${percent === undefined ? '' : ` · ${percent}%`}`,
    body: 'Description', commits: 'Commits', journal: 'History', noJournal: 'No history yet', work: 'Work log', subtasks: 'Subtasks',
    kinds: { issue: 'task', epic: 'epic', milestone: 'milestone', backlog: 'backlog' },
    priority: (n) => `P${n}`, created_entry: 'Created', problem: 'The details could not be read.', close: 'Close details',
  },
  ko: {
    label: '상세',
    none: '할 일을 고르면 상세가 여기에 나옵니다.',
    assignee: '담당자', created: '만든 시각', updated: '고친 시각', started: '시작', ended: '끝남', due: '마감', epic: '에픽', milestone: '마일스톤',
    blockedBy: '막는 것', blocks: '막고 있는 것', deferred: '미룸',
    deferredUnder: (id) => `미룸 — ${id} 아래`,
    blockerDone: '끝남', blockerOpen: '열림', blockerDeferred: '미룸', blockerMissing: '없음',
    rollup: '진척', rollupText: (done, work, percent) => `${done}/${work} 완료${percent === undefined ? '' : ` · ${percent}%`}`,
    body: '본문', commits: '커밋', journal: '기록', noJournal: '아직 기록이 없습니다', work: '작업 기록', subtasks: '하위 항목',
    kinds: { issue: '할 일', epic: '에픽', milestone: '마일스톤', backlog: '백로그' },
    priority: (n) => `P${n}`, created_entry: '만들었습니다', problem: '상세를 읽지 못했습니다.', close: '상세 닫기',
  },
  ja: {
    label: '詳細',
    none: 'タスクを選ぶと、ここに詳細が表示されます。',
    assignee: '担当者', created: '作成', updated: '更新', started: '開始', ended: '終了', due: '期限', epic: 'エピック', milestone: 'マイルストーン',
    blockedBy: 'ブロック元', blocks: 'ブロック先', deferred: '保留',
    deferredUnder: (id) => `保留 — ${id} の配下`,
    blockerDone: '完了', blockerOpen: '未完了', blockerDeferred: '保留', blockerMissing: '見つからない',
    rollup: '進捗', rollupText: (done, work, percent) => `${done}/${work} 完了${percent === undefined ? '' : ` · ${percent}%`}`,
    body: '本文', commits: 'コミット', journal: '履歴', noJournal: '履歴はまだありません', work: '作業ログ', subtasks: 'サブタスク',
    kinds: { issue: 'タスク', epic: 'エピック', milestone: 'マイルストーン', backlog: 'バックログ' },
    priority: (n) => `P${n}`, created_entry: '作成しました', problem: '詳細を読み込めませんでした。', close: '詳細を閉じる',
  },
  zh: {
    label: '详情',
    none: '选择任务后，详情会显示在这里。',
    assignee: '负责人', created: '创建', updated: '更新', started: '开始', ended: '结束', due: '截止', epic: '史诗', milestone: '里程碑',
    blockedBy: '被阻塞于', blocks: '阻塞', deferred: '已搁置',
    deferredUnder: (id) => `已搁置 — 隶属于 ${id}`,
    blockerDone: '已完成', blockerOpen: '未完成', blockerDeferred: '已搁置', blockerMissing: '未找到',
    rollup: '进度', rollupText: (done, work, percent) => `${done}/${work} 完成${percent === undefined ? '' : ` · ${percent}%`}`,
    body: '正文', commits: '提交', journal: '历史', noJournal: '暂无历史', work: '工作记录', subtasks: '子任务',
    kinds: { issue: '任务', epic: '史诗', milestone: '里程碑', backlog: '待办池' },
    priority: (n) => `P${n}`, created_entry: '已创建', problem: '无法读取详情。', close: '关闭详情',
  },
}

// ── toolbar: search, filter and sort above the list and board (E4 se-cv7w) ───────────────────────────

/** Why the list would hide a row that a search shows (view/list-model.ts viewHides): the view toggle that hides it. */
export type HiddenReason = 'column' | 'deferred' | 'backlog' | 'archived'

export interface ToolbarCopy {
  search: string
  searchPlaceholder: string
  /** Accessible name of the search scope select. */
  scope: string
  scopes: Record<GrepScope, string>
  /** In the scope select: rows carry no notes, so no scope searches them (se-thyl). */
  notesUnsearched: string
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
  hidden: (n: number) => string
  hiddenReasons: Record<HiddenReason, string>
  /** The sort editor's note on `D`: the default it saves lasts for this session (keys/wiring.ts hides it where it persists). */
  sortDefaultSessionNote: string
}

const TOOLBAR: Record<Language, ToolbarCopy> = {
  en: {
    search: 'Search', searchPlaceholder: 'Search (/)', scope: 'Search in',
    scopes: { all: 'All', id: 'ID', title: 'Title', tag: 'Tag', body: 'Body', note: 'Note' },
    notesUnsearched: 'Notes are not searched',
    filter: 'Filter', filterPlaceholder: 'status=todo tag=bug,ui (Enter applies)', removeChip: 'Remove filter item', hints: 'Suggestions',
    sort: 'Sort', sortEdit: 'Edit sort', reversed: 'reversed', replace: 'Only this', append: 'Add', flip: 'Flip direction',
    sortKeys: { priority: 'Priority', created: 'Created', updated: 'Updated', status: 'Status', assignee: 'Assignee', title: 'Title', id: 'ID' },
    hidden: (n) => `${n} hidden`,
    hiddenReasons: { column: 'hidden column', deferred: 'deferred', backlog: 'backlog', archived: 'archived' },
    sortDefaultSessionNote: 'D: save as the default order — kept for this session only.',
  },
  ko: {
    search: '검색', searchPlaceholder: '검색 (/)', scope: '검색 범위',
    scopes: { all: '전체', id: 'ID', title: '제목', tag: '태그', body: '본문', note: '메모' },
    notesUnsearched: '메모는 검색하지 않습니다',
    filter: '필터', filterPlaceholder: 'status=todo tag=bug,ui (Enter로 적용)', removeChip: '필터 항목 지우기', hints: '자동 완성',
    sort: '정렬', sortEdit: '정렬 편집', reversed: '뒤집힘', replace: '이것만', append: '추가', flip: '방향 바꿈',
    sortKeys: { priority: '우선순위', created: '만든 때', updated: '고친 때', status: '상태', assignee: '담당', title: '제목', id: 'ID' },
    hidden: (n) => `${n}건 숨김`,
    hiddenReasons: { column: '숨긴 칸', deferred: '미룸', backlog: '백로그', archived: '보관' },
    sortDefaultSessionNote: 'D: 기본 정렬로 저장 — 이 세션에서만 유지됩니다.',
  },
  ja: {
    search: '検索', searchPlaceholder: '検索 (/)', scope: '検索範囲',
    scopes: { all: 'すべて', id: 'ID', title: 'タイトル', tag: 'タグ', body: '本文', note: 'メモ' },
    notesUnsearched: 'メモは検索しません',
    filter: 'フィルター', filterPlaceholder: 'status=todo tag=bug,ui (Enter で適用)', removeChip: 'フィルター項目を削除', hints: '候補',
    sort: '並べ替え', sortEdit: '並べ替えを編集', reversed: '逆順', replace: 'これだけ', append: '追加', flip: '向きを反転',
    sortKeys: { priority: '優先度', created: '作成', updated: '更新', status: '状態', assignee: '担当', title: 'タイトル', id: 'ID' },
    hidden: (n) => `${n} 件を非表示`,
    hiddenReasons: { column: '非表示の列', deferred: '保留', backlog: 'バックログ', archived: 'アーカイブ' },
    sortDefaultSessionNote: 'D: 既定の並べ替えとして保存 — このセッションの間だけ保持されます。',
  },
  zh: {
    search: '搜索', searchPlaceholder: '搜索 (/)', scope: '搜索范围',
    scopes: { all: '全部', id: 'ID', title: '标题', tag: '标签', body: '正文', note: '备注' },
    notesUnsearched: '不搜索备注',
    filter: '筛选', filterPlaceholder: 'status=todo tag=bug,ui (按 Enter 应用)', removeChip: '删除筛选项', hints: '建议',
    sort: '排序', sortEdit: '编辑排序', reversed: '反向', replace: '仅此项', append: '添加', flip: '反转方向',
    sortKeys: { priority: '优先级', created: '创建', updated: '更新', status: '状态', assignee: '负责人', title: '标题', id: 'ID' },
    hidden: (n) => `已隐藏 ${n} 项`,
    hiddenReasons: { column: '隐藏的列', deferred: '推迟', backlog: '待办池', archived: '已归档' },
    sortDefaultSessionNote: 'D: 保存为默认排序 — 仅在本次会话中保留。',
  },
}

// ── keys: the key router's words (se-jwl3); the key table's own labels are the `table` group below ───────

export interface KeysCopy {
  /** Shown while the panel's frame has no keyboard focus (the host does not give it one). */
  focusHint: string
  /** `r`, `SPC m a`, `SPC m g` in a panel given no read-mark writes (keys/wiring.ts `read`). */
  readUnavailable: string
  /** `SPC m g` with neither a group under the cursor nor a group to stand in. */
  readNoGroup: string
  /** `D` in the sort editor: the default order is saved where it outlives the panel. */
  sortDefaultSaved: string
  /** `D` in the sort editor while no settings storage keeps it: saved for this session only. */
  sortDefaultSavedSession: string
}

const KEYS: Record<Language, KeysCopy> = {
  en: {
    focusHint: 'Click to use the keyboard',
    readUnavailable: 'Marking items read is not available in the panel yet.',
    readNoGroup: 'Move to a milestone or epic to mark its items read.',
    sortDefaultSaved: 'Saved as the default order.',
    sortDefaultSavedSession: 'Saved as the default order. Until a settings storage is available it is kept for this session only.',
  },
  ko: {
    focusHint: '클릭하면 키보드를 쓸 수 있습니다',
    readUnavailable: '읽음 표시는 아직 패널에서 쓸 수 없습니다.',
    readNoGroup: '읽음으로 표시할 마일스톤이나 에픽으로 옮겨 가세요.',
    sortDefaultSaved: '기본 정렬로 저장했습니다.',
    sortDefaultSavedSession: '기본 정렬로 저장했습니다. 설정 저장소가 생기기 전까지는 이 세션에서만 유지됩니다.',
  },
  ja: {
    focusHint: 'クリックするとキーボードを使えます',
    readUnavailable: '既読の記録はまだパネルでは使えません。',
    readNoGroup: '既読にするマイルストーンかエピックに移動してください。',
    sortDefaultSaved: '既定の並べ替えとして保存しました。',
    sortDefaultSavedSession: '既定の並べ替えとして保存しました。設定の保存先ができるまでは、このセッションの間だけ保持されます。',
  },
  zh: {
    focusHint: '点击后即可使用键盘',
    readUnavailable: '面板中暂时还不能标记为已读。',
    readNoGroup: '请移到要标记为已读的里程碑或史诗。',
    sortDefaultSaved: '已保存为默认排序。',
    sortDefaultSavedSession: '已保存为默认排序。在有设置存储之前，仅在本次会话中保留。',
  },
}

// ── table: the key table's words — SPC menu, help overlay (keys/table.ts, view/menu.ts, view/help.ts) ────────

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
  sortAdd: (field: string) => string
  /** The help overlay's longer words for the same item (the menu's cell is narrow). */
  sortAddHelp: (field: string) => string
  /** `SPC v N` when the board has fewer status columns named than N. */
  column: (n: number) => string
  /** The help row standing for `SPC v 1~9`. */
  columns: string
  /** The help overlay's head. */
  helpTitle: string
  close: string
}

const TABLE_EN: KeyTableCopy = {
  labels: {
    'move.down': 'One line down', 'move.up': 'One line up', 'move.top': 'Top', 'move.bottom': 'Bottom',
    'move.halfDown': 'Half a page down (cursor stays)', 'move.halfUp': 'Half a page up (cursor stays)',
    'move.pageDown': 'One page down', 'move.pageUp': 'One page up',
    'wiki.halfDown': 'Half a page down', 'wiki.halfUp': 'Half a page up',
    'focus.next': 'Next pane', 'focus.prev': 'Previous pane', 'focus.left': 'Left pane', 'focus.right': 'Right pane', 'focus.up': 'Pane above', 'focus.down': 'Pane below',
    enter: 'Go in (a leaf row opens its details)', leave: 'Up one level',
    expand: 'Expand (board: column to the right)', collapse: 'Collapse (board: column to the left)', expandAll: 'Expand or collapse all under a group',
    search: 'Search', clear: 'Clear the search and filter', read: 'Mark the cursor row read', help: 'Keyboard shortcuts',
    filter: 'Filter', 'tab.list': 'List', 'tab.board': 'Board', 'tab.wiki': 'Wiki',
    'view.deferred': 'Deferred', 'view.backlog': 'Backlog', 'view.archived': 'Archive',
    'view.all': 'All but archived', 'view.allHelp': 'Show all (archive stays hidden)',
    'view.detail': 'Details', 'view.raw': 'Raw', 'view.rawHelp': 'Raw or rendered', 'sort.edit': 'Edit the order…',
    'read.all': 'Mark all read', 'read.group': 'Group read', 'read.groupHelp': 'Mark the group read', 'option.detailAt': 'Details at', 'option.detailAtHelp': 'Details position (left ↔ bottom)',
    'menu.close': 'Close', 'menu.up': 'Up one level',
    'prompt.apply': 'Apply (with filter values listed, insert one)', 'prompt.cancel': 'Cancel, back to the cursor before',
    'prompt.next': 'Cycle the search scope / complete the filter', 'prompt.prev': 'Cycle the scope back / complete back',
    'prompt.up': 'Previous filter value', 'prompt.down': 'Next filter value', 'prompt.clear': 'Clear the whole input', 'prompt.deleteWord': 'Delete a word',
    'sorter.down': 'Down', 'sorter.up': 'Up', 'sorter.lower': 'Move later', 'sorter.raise': 'Move earlier', 'sorter.toggle': 'Use or drop',
    'sorter.flip': 'Flip direction', 'sorter.apply': 'Apply', 'sorter.default': 'Save as default', 'sorter.cancel': 'Cancel',
    'wiki.enter': 'Open the page / pick or follow a link', 'wiki.back': 'Go back the way you came', 'wiki.page': 'From the list to the page',
    'wiki.nextLink': 'Next link', 'wiki.prevLink': 'Previous link', 'wiki.search': 'Search titles and text', 'wiki.clear': 'Drop the link, then the search',
  },
  root: 'Menu',
  layers: { g: 'Go', v: 'View', s: 'Sort', c: 'Columns', m: 'Read', o: 'Options', w: 'Window' },
  groups: {
    move: 'Move', enter: 'In, out and expand', wiki: 'Wiki', focus: 'Pane focus', tab: 'Switch views', search: 'Search and filter',
    read: 'Read', view: 'View (SPC v)', sort: 'Sort (SPC s)', cell: 'Columns (SPC c)', option: 'Options (SPC o)', other: 'Other',
    menu: 'Inside the SPC menu', prompt: 'Search and filter input', sorter: 'Sort editor',
  },
  states: {
    shown: '[shown]', hidden: '[hidden]', asc: '[↑ asc]', desc: '[↓ desc]', left: '[left]', bottom: '[bottom]',
    raw: '[raw]', rendered: '[rendered]', list: '[list]', board: '[board]', wiki: '[wiki]',
  },
  sortFields: { priority: 'Priority', created: 'Created', updated: 'Updated', status: 'Status', assignee: 'Assignee', title: 'Title' },
  cellFields: { id: 'id', priority: 'Priority', assignee: 'Assignee', created: 'Created', updated: 'Updated', counts: 'Counts (n/n)', tags: 'Tags', epic: 'Epic', names: 'Name line' },
  sortAdd: (field) => `${field} (add)`,
  sortAddHelp: (field) => `${field} (add at the end)`,
  column: (n) => `Column ${n}`,
  columns: 'Columns 1–9',
  helpTitle: 'Shortcuts',
  close: 'Close',
}

const TABLE_KO: KeyTableCopy = {
  labels: {
    'move.down': '한 줄 아래', 'move.up': '한 줄 위', 'move.top': '맨 위', 'move.bottom': '맨 아래',
    'move.halfDown': '반 페이지 아래로 (커서 그대로)', 'move.halfUp': '반 페이지 위로 (커서 그대로)',
    'move.pageDown': '한 페이지 아래로', 'move.pageUp': '한 페이지 위로',
    'wiki.halfDown': '반 페이지 아래로', 'wiki.halfUp': '반 페이지 위로',
    'focus.next': '다음 패널', 'focus.prev': '이전 패널', 'focus.left': '왼쪽 패널', 'focus.right': '오른쪽 패널', 'focus.up': '위 패널', 'focus.down': '아래 패널',
    enter: '들어가기 (잎 행이면 상세로)', leave: '한 층 나가기',
    expand: '펼침 (칸반: 오른쪽 칸)', collapse: '접음 (칸반: 왼쪽 칸)', expandAll: '그룹 밑을 모두 펼침·접음',
    search: '검색', clear: '걸린 검색·필터 해제', read: '커서 행 읽음', help: '단축키 도움말',
    filter: '필터', 'tab.list': '목록', 'tab.board': '칸반', 'tab.wiki': '위키',
    'view.deferred': '미룸', 'view.backlog': '백로그', 'view.archived': '아카이브',
    'view.all': '아카이브 빼고 모두', 'view.allHelp': '모두 보이기 (아카이브 제외)',
    'view.detail': '상세', 'view.raw': '원문', 'view.rawHelp': '원문 또는 렌더링', 'sort.edit': '정렬 편집…',
    'read.all': '모두 읽음', 'read.group': '그룹 읽음', 'read.groupHelp': '그룹을 읽음으로', 'option.detailAt': '상세 위치', 'option.detailAtHelp': '상세 위치 (왼쪽 ↔ 아래)',
    'menu.close': '닫기', 'menu.up': '한 층 위로',
    'prompt.apply': '적용 (필터 값 목록이 있으면 값 넣기)', 'prompt.cancel': '취소하고 열기 전 커서로',
    'prompt.next': '검색 범위 순환 / 필터 자동완성', 'prompt.prev': '검색 범위 거꾸로 / 필터 자동완성 거꾸로',
    'prompt.up': '필터 값 후보 위', 'prompt.down': '필터 값 후보 아래', 'prompt.clear': '입력 전체 지움', 'prompt.deleteWord': '단어 지움',
    'sorter.down': '아래로', 'sorter.up': '위로', 'sorter.lower': '순서 뒤로', 'sorter.raise': '순서 앞으로', 'sorter.toggle': '넣기·빼기',
    'sorter.flip': '방향 반전', 'sorter.apply': '적용', 'sorter.default': '기본값으로 저장', 'sorter.cancel': '취소',
    'wiki.enter': '페이지 열기 / 링크 선택·이동', 'wiki.back': '건너온 길 되감기', 'wiki.page': '목록에서 본문으로',
    'wiki.nextLink': '다음 링크', 'wiki.prevLink': '이전 링크', 'wiki.search': '제목·본문 검색', 'wiki.clear': '링크 선택 해제 → 검색 해제',
  },
  root: '메뉴',
  layers: { g: '이동', v: '보기', s: '정렬', c: '열', m: '읽음', o: '옵션', w: '창' },
  groups: {
    move: '이동', enter: '드나들기·펼침', wiki: '위키', focus: '패널 포커스', tab: '화면 전환', search: '검색·필터',
    read: '읽음', view: '보기 (SPC v)', sort: '정렬 (SPC s)', cell: '열 (SPC c)', option: '옵션 (SPC o)', other: '기타',
    menu: 'SPC 메뉴 안에서', prompt: '검색·필터 입력', sorter: '정렬 편집',
  },
  states: {
    shown: '[보임]', hidden: '[숨김]', asc: '[↑ 오름]', desc: '[↓ 내림]', left: '[왼쪽]', bottom: '[아래]',
    raw: '[원문]', rendered: '[렌더링]', list: '[목록]', board: '[칸반]', wiki: '[위키]',
  },
  sortFields: { priority: '우선순위', created: '생성일', updated: '수정일', status: '상태', assignee: '담당자', title: '제목' },
  cellFields: { id: 'id', priority: '우선순위', assignee: '담당자', created: '생성일', updated: '수정일', counts: '셈 (n/n)', tags: '태그', epic: '에픽', names: '이름줄' },
  sortAdd: (field) => `${field} (추가)`,
  sortAddHelp: (field) => `${field} (끝에 추가)`,
  column: (n) => `${n}번째 칸`,
  columns: '1~9번째 칸',
  helpTitle: '단축키',
  close: '닫기',
}

const TABLE_JA: KeyTableCopy = {
  labels: {
    'move.down': '1 行下', 'move.up': '1 行上', 'move.top': '先頭', 'move.bottom': '末尾',
    'move.halfDown': '半ページ下へ (カーソルはそのまま)', 'move.halfUp': '半ページ上へ (カーソルはそのまま)',
    'move.pageDown': '1 ページ下へ', 'move.pageUp': '1 ページ上へ',
    'wiki.halfDown': '半ページ下へ', 'wiki.halfUp': '半ページ上へ',
    'focus.next': '次のペイン', 'focus.prev': '前のペイン', 'focus.left': '左のペイン', 'focus.right': '右のペイン', 'focus.up': '上のペイン', 'focus.down': '下のペイン',
    enter: '入る (末端の行なら詳細へ)', leave: '一つ上の階層へ',
    expand: '展開 (カンバン: 右の列)', collapse: '折りたたむ (カンバン: 左の列)', expandAll: 'グループの下をすべて展開・折りたたみ',
    search: '検索', clear: '検索・フィルターを解除', read: 'カーソル行を既読に', help: 'ショートカット',
    filter: 'フィルター', 'tab.list': '一覧', 'tab.board': 'カンバン', 'tab.wiki': 'Wiki',
    'view.deferred': '保留', 'view.backlog': 'バックログ', 'view.archived': 'アーカイブ',
    'view.all': 'アーカイブ以外', 'view.allHelp': 'すべて表示 (アーカイブは除く)',
    'view.detail': '詳細', 'view.raw': '原文', 'view.rawHelp': '原文またはレンダリング', 'sort.edit': '並べ替えを編集…',
    'read.all': 'すべて既読', 'read.group': 'グループ既読', 'read.groupHelp': 'グループを既読に', 'option.detailAt': '詳細の位置', 'option.detailAtHelp': '詳細の位置 (左 ↔ 下)',
    'menu.close': '閉じる', 'menu.up': '一つ上の階層へ',
    'prompt.apply': '適用 (フィルター値の候補があれば挿入)', 'prompt.cancel': 'キャンセルして開く前のカーソルへ',
    'prompt.next': '検索範囲を切り替え / フィルターを補完', 'prompt.prev': '検索範囲を逆に切り替え / 逆に補完',
    'prompt.up': 'フィルター値の前の候補', 'prompt.down': 'フィルター値の次の候補', 'prompt.clear': '入力をすべて消去', 'prompt.deleteWord': '単語を削除',
    'sorter.down': '下へ', 'sorter.up': '上へ', 'sorter.lower': '順序を後ろへ', 'sorter.raise': '順序を前へ', 'sorter.toggle': '入れる・外す',
    'sorter.flip': '向きを反転', 'sorter.apply': '適用', 'sorter.default': '既定として保存', 'sorter.cancel': 'キャンセル',
    'wiki.enter': 'ページを開く / リンクを選択・移動', 'wiki.back': '来た道を戻る', 'wiki.page': '一覧から本文へ',
    'wiki.nextLink': '次のリンク', 'wiki.prevLink': '前のリンク', 'wiki.search': 'タイトル・本文を検索', 'wiki.clear': 'リンクの選択を解除 → 検索を解除',
  },
  root: 'メニュー',
  layers: { g: '移動', v: '表示', s: '並べ替え', c: '列', m: '既読', o: 'オプション', w: 'ウィンドウ' },
  groups: {
    move: '移動', enter: '出入り・展開', wiki: 'Wiki', focus: 'ペインのフォーカス', tab: '画面の切り替え', search: '検索・フィルター',
    read: '既読', view: '表示 (SPC v)', sort: '並べ替え (SPC s)', cell: '列 (SPC c)', option: 'オプション (SPC o)', other: 'その他',
    menu: 'SPC メニューの中', prompt: '検索・フィルターの入力', sorter: '並べ替えの編集',
  },
  states: {
    shown: '[表示]', hidden: '[非表示]', asc: '[↑ 昇順]', desc: '[↓ 降順]', left: '[左]', bottom: '[下]',
    raw: '[原文]', rendered: '[レンダリング]', list: '[一覧]', board: '[カンバン]', wiki: '[Wiki]',
  },
  sortFields: { priority: '優先度', created: '作成日', updated: '更新日', status: '状態', assignee: '担当者', title: 'タイトル' },
  cellFields: { id: 'id', priority: '優先度', assignee: '担当者', created: '作成日', updated: '更新日', counts: '件数 (n/n)', tags: 'タグ', epic: 'エピック', names: '名前の行' },
  sortAdd: (field) => `${field} (追加)`,
  sortAddHelp: (field) => `${field} (末尾に追加)`,
  column: (n) => `${n} 番目の列`,
  columns: '1〜9 番目の列',
  helpTitle: 'ショートカット',
  close: '閉じる',
}

const TABLE_ZH: KeyTableCopy = {
  labels: {
    'move.down': '下移一行', 'move.up': '上移一行', 'move.top': '顶部', 'move.bottom': '底部',
    'move.halfDown': '向下半页 (光标不动)', 'move.halfUp': '向上半页 (光标不动)',
    'move.pageDown': '向下一页', 'move.pageUp': '向上一页',
    'wiki.halfDown': '向下半页', 'wiki.halfUp': '向上半页',
    'focus.next': '下一个窗格', 'focus.prev': '上一个窗格', 'focus.left': '左侧窗格', 'focus.right': '右侧窗格', 'focus.up': '上方窗格', 'focus.down': '下方窗格',
    enter: '进入 (叶子行则打开详情)', leave: '返回上一层',
    expand: '展开 (看板: 右侧列)', collapse: '折叠 (看板: 左侧列)', expandAll: '展开或折叠分组下的全部',
    search: '搜索', clear: '清除搜索和筛选', read: '将光标所在行标为已读', help: '键盘快捷键',
    filter: '筛选', 'tab.list': '列表', 'tab.board': '看板', 'tab.wiki': 'Wiki',
    'view.deferred': '推迟', 'view.backlog': '待办池', 'view.archived': '归档',
    'view.all': '除归档外全部', 'view.allHelp': '全部显示 (归档除外)',
    'view.detail': '详情', 'view.raw': '原文', 'view.rawHelp': '原文或渲染', 'sort.edit': '编辑排序…',
    'read.all': '全部标为已读', 'read.group': '分组已读', 'read.groupHelp': '分组标为已读', 'option.detailAt': '详情位置', 'option.detailAtHelp': '详情位置 (左 ↔ 下)',
    'menu.close': '关闭', 'menu.up': '返回上一层',
    'prompt.apply': '应用 (有筛选值候选时插入)', 'prompt.cancel': '取消并回到打开前的光标',
    'prompt.next': '切换搜索范围 / 补全筛选', 'prompt.prev': '反向切换搜索范围 / 反向补全',
    'prompt.up': '上一个筛选值候选', 'prompt.down': '下一个筛选值候选', 'prompt.clear': '清空全部输入', 'prompt.deleteWord': '删除一个词',
    'sorter.down': '向下', 'sorter.up': '向上', 'sorter.lower': '顺序后移', 'sorter.raise': '顺序前移', 'sorter.toggle': '加入·移除',
    'sorter.flip': '反转方向', 'sorter.apply': '应用', 'sorter.default': '保存为默认', 'sorter.cancel': '取消',
    'wiki.enter': '打开页面 / 选择或跟随链接', 'wiki.back': '沿来路返回', 'wiki.page': '从列表到正文',
    'wiki.nextLink': '下一个链接', 'wiki.prevLink': '上一个链接', 'wiki.search': '搜索标题和正文', 'wiki.clear': '取消链接选择 → 清除搜索',
  },
  root: '菜单',
  layers: { g: '跳转', v: '视图', s: '排序', c: '列', m: '已读', o: '选项', w: '窗口' },
  groups: {
    move: '移动', enter: '进出·展开', wiki: 'Wiki', focus: '窗格焦点', tab: '切换视图', search: '搜索·筛选',
    read: '已读', view: '视图 (SPC v)', sort: '排序 (SPC s)', cell: '列 (SPC c)', option: '选项 (SPC o)', other: '其他',
    menu: 'SPC 菜单内', prompt: '搜索·筛选输入', sorter: '编辑排序',
  },
  states: {
    shown: '[显示]', hidden: '[隐藏]', asc: '[↑ 升序]', desc: '[↓ 降序]', left: '[左]', bottom: '[下]',
    raw: '[原文]', rendered: '[渲染]', list: '[列表]', board: '[看板]', wiki: '[Wiki]',
  },
  sortFields: { priority: '优先级', created: '创建日期', updated: '更新日期', status: '状态', assignee: '负责人', title: '标题' },
  cellFields: { id: 'id', priority: '优先级', assignee: '负责人', created: '创建日期', updated: '更新日期', counts: '计数 (n/n)', tags: '标签', epic: '史诗', names: '名称行' },
  sortAdd: (field) => `${field} (追加)`,
  sortAddHelp: (field) => `${field} (追加到末尾)`,
  column: (n) => `第 ${n} 列`,
  columns: '第 1~9 列',
  helpTitle: '快捷键',
  close: '关闭',
}

export const TABLE: Record<Language, KeyTableCopy> = { en: TABLE_EN, ko: TABLE_KO, ja: TABLE_JA, zh: TABLE_ZH }

// ── wiki: the wiki tab (E7 se-rhu3, view/wiki.ts) ────────────────────────────────────────────────────

export interface WikiCopy {
  /** The reads' answers could not be used; the parse error or the shape follows. */
  badJson: (reason: string) => string
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
  noPagesHint: (dir: string) => string
  noMatch: string
  noMatchHint: string
  backlinksLabel: string
  backlinks: (n: number) => string
  noBacklinks: string
  backlinksPartial: string
  choosePage: string
  choosePageHint: string
  loadingPage: string
  pageConflict: string
  bodyFailed: string
  renderFailed: (reason: string) => string
  /** `#anchor` in raw view, where headings are not targets. */
  rawNoAnchor: (anchor: string) => string
  noAnchor: (anchor: string) => string
  noLinks: string
  /** `tui.wiki.outside`. */
  outside: (href: string) => string
  loadFailed: (reason: string) => string
  nothingBack: string
}

const WIKI: Record<Language, WikiCopy> = {
  en: {
    badJson: (reason) => `The wiki's answer could not be read: ${reason}`,
    listShape: 'The wiki list answer is not in the expected shape.',
    listNoPages: 'The wiki list answer has no pages.',
    folderFailed: 'The wiki folder could not be read.',
    pageFailed: 'The wiki page could not be read.',
    pageShape: 'The wiki page answer is not in the expected shape.',
    conflict: 'Conflict', conflictDetail: 'Git conflict markers are left in it.',
    unread: 'Not read', large: 'Over 1 MB', largeDetail: 'Over 1 MB, so its text is not read.',
    searchPlaceholder: 'Search the wiki', searchLabel: 'Search wiki titles and text', pagesLabel: 'Wiki pages',
    back: 'Back', raw: 'Raw', rawTitle: 'Raw or rendered (SPC v r)', bodyLabel: 'Wiki page', resizer: 'Resize the wiki list',
    loadingList: 'Loading the wiki', noPages: 'No wiki pages',
    noPagesHint: (dir) => `Put .md files in ${dir} and they show up here.`,
    noMatch: 'No page matches', noMatchHint: 'Esc clears the search.',
    backlinksLabel: 'Backlinks', backlinks: (n) => `Backlinks ${n}`,
    noBacklinks: 'No page links here.', backlinksPartial: 'Some pages could not be read, so this may count too few.',
    choosePage: 'Choose a page', choosePageHint: 'Enter in the list opens it.', loadingPage: 'Loading the page',
    pageConflict: 'Git conflict markers are left in this page.', bodyFailed: 'The page text could not be read.',
    renderFailed: (reason) => `It could not be rendered, so the raw text is shown: ${reason}`,
    rawNoAnchor: (anchor) => `The raw view does not go to the heading '${anchor}'.`,
    noAnchor: (anchor) => `This page has no heading '${anchor}'.`,
    noLinks: 'This page has no links.',
    outside: (href) => `Outside the wiki, not opened: ${href}`,
    loadFailed: (reason) => `The page could not be loaded: ${reason}`,
    nothingBack: 'There is no page to go back to.',
  },
  ko: {
    badJson: (reason) => `위키 응답을 읽지 못했습니다: ${reason}`,
    listShape: '위키 목록 응답의 꼴이 맞지 않습니다.',
    listNoPages: '위키 목록 응답에 pages가 없습니다.',
    folderFailed: '위키 폴더를 읽지 못했습니다.',
    pageFailed: '위키 페이지를 읽지 못했습니다.',
    pageShape: '위키 페이지 응답의 꼴이 맞지 않습니다.',
    conflict: '충돌', conflictDetail: 'git 충돌 표시가 남아 있습니다.',
    unread: '못 읽음', large: '1MB 초과', largeDetail: '1MB가 넘어 본문을 읽지 않습니다.',
    searchPlaceholder: '위키 검색', searchLabel: '위키 제목과 본문 검색', pagesLabel: '위키 페이지',
    back: '되감기', raw: '원문', rawTitle: '원문·렌더링 (SPC v r)', bodyLabel: '위키 본문', resizer: '위키 목록 폭 조절',
    loadingList: '위키를 불러오는 중입니다', noPages: '위키 페이지가 없습니다',
    noPagesHint: (dir) => `${dir} 폴더에 .md 파일을 두면 여기에 섭니다.`,
    noMatch: '찾는 페이지가 없습니다', noMatchHint: 'Esc로 검색을 지웁니다.',
    backlinksLabel: '역링크', backlinks: (n) => `역링크 ${n}`,
    noBacklinks: '이 페이지를 가리키는 문서가 없습니다.', backlinksPartial: '읽지 못한 페이지가 있어 덜 셌을 수 있습니다.',
    choosePage: '페이지를 고르세요', choosePageHint: '목록에서 Enter로 엽니다.', loadingPage: '페이지를 불러오는 중입니다',
    pageConflict: '이 페이지에 git 충돌 표시가 남아 있습니다.', bodyFailed: '본문을 읽지 못했습니다.',
    renderFailed: (reason) => `렌더링하지 못해 원문을 보여 줍니다: ${reason}`,
    rawNoAnchor: (anchor) => `원문 보기에서는 '${anchor}' 제목으로 가지 않습니다.`,
    noAnchor: (anchor) => `이 페이지에 '${anchor}' 제목이 없습니다.`,
    noLinks: '이 페이지에는 링크가 없습니다.',
    outside: (href) => `위키 밖 주소는 열지 않습니다: ${href}`,
    loadFailed: (reason) => `페이지를 불러오지 못했습니다: ${reason}`,
    nothingBack: '되감을 페이지가 없습니다.',
  },
  ja: {
    badJson: (reason) => `Wiki の応答を読めませんでした: ${reason}`,
    listShape: 'Wiki 一覧の応答の形式が正しくありません。',
    listNoPages: 'Wiki 一覧の応答に pages がありません。',
    folderFailed: 'Wiki のフォルダを読めませんでした。',
    pageFailed: 'Wiki のページを読めませんでした。',
    pageShape: 'Wiki ページの応答の形式が正しくありません。',
    conflict: '競合', conflictDetail: 'git の競合マーカーが残っています。',
    unread: '読めない', large: '1MB 超', largeDetail: '1MB を超えるため本文を読みません。',
    searchPlaceholder: 'Wiki を検索', searchLabel: 'Wiki のタイトルと本文を検索', pagesLabel: 'Wiki のページ',
    back: '戻る', raw: '原文', rawTitle: '原文・レンダリング (SPC v r)', bodyLabel: 'Wiki の本文', resizer: 'Wiki 一覧の幅を変更',
    loadingList: 'Wiki を読み込んでいます', noPages: 'Wiki のページがありません',
    noPagesHint: (dir) => `${dir} フォルダに .md ファイルを置くと、ここに表示されます。`,
    noMatch: '該当するページがありません', noMatchHint: 'Esc で検索を消します。',
    backlinksLabel: 'バックリンク', backlinks: (n) => `バックリンク ${n}`,
    noBacklinks: 'このページを指すページはありません。', backlinksPartial: '読めなかったページがあるため、少なく数えている可能性があります。',
    choosePage: 'ページを選んでください', choosePageHint: '一覧で Enter を押すと開きます。', loadingPage: 'ページを読み込んでいます',
    pageConflict: 'このページには git の競合マーカーが残っています。', bodyFailed: '本文を読めませんでした。',
    renderFailed: (reason) => `レンダリングできなかったため、原文を表示しています: ${reason}`,
    rawNoAnchor: (anchor) => `原文表示では見出し「${anchor}」へ移動しません。`,
    noAnchor: (anchor) => `このページに見出し「${anchor}」はありません。`,
    noLinks: 'このページにはリンクがありません。',
    outside: (href) => `Wiki の外のアドレスは開きません: ${href}`,
    loadFailed: (reason) => `ページを読み込めませんでした: ${reason}`,
    nothingBack: '戻るページがありません。',
  },
  zh: {
    badJson: (reason) => `无法读取 Wiki 的响应: ${reason}`,
    listShape: 'Wiki 列表响应的格式不正确。',
    listNoPages: 'Wiki 列表响应中没有 pages。',
    folderFailed: '无法读取 Wiki 文件夹。',
    pageFailed: '无法读取 Wiki 页面。',
    pageShape: 'Wiki 页面响应的格式不正确。',
    conflict: '冲突', conflictDetail: '仍留有 git 冲突标记。',
    unread: '无法读取', large: '超过 1MB', largeDetail: '超过 1MB，不读取正文。',
    searchPlaceholder: '搜索 Wiki', searchLabel: '搜索 Wiki 标题和正文', pagesLabel: 'Wiki 页面',
    back: '返回', raw: '原文', rawTitle: '原文·渲染 (SPC v r)', bodyLabel: 'Wiki 正文', resizer: '调整 Wiki 列表宽度',
    loadingList: '正在加载 Wiki', noPages: '没有 Wiki 页面',
    noPagesHint: (dir) => `在 ${dir} 文件夹中放入 .md 文件后会显示在这里。`,
    noMatch: '没有匹配的页面', noMatchHint: '按 Esc 清除搜索。',
    backlinksLabel: '反向链接', backlinks: (n) => `反向链接 ${n}`,
    noBacklinks: '没有指向此页面的文档。', backlinksPartial: '有页面无法读取，计数可能偏少。',
    choosePage: '请选择页面', choosePageHint: '在列表中按 Enter 打开。', loadingPage: '正在加载页面',
    pageConflict: '此页面仍留有 git 冲突标记。', bodyFailed: '无法读取正文。',
    renderFailed: (reason) => `无法渲染，因此显示原文: ${reason}`,
    rawNoAnchor: (anchor) => `原文视图中不会跳转到标题“${anchor}”。`,
    noAnchor: (anchor) => `此页面没有标题“${anchor}”。`,
    noLinks: '此页面没有链接。',
    outside: (href) => `不打开 Wiki 以外的地址: ${href}`,
    loadFailed: (reason) => `无法加载页面: ${reason}`,
    nothingBack: '没有可返回的页面。',
  },
}

// ── the 1.x words, and the groups put together ───────────────────────────────────────────────────────

export interface Copy {
  hint: string
  refreshedAt: (time: string) => string
  empty: string
  more: (n: number) => string
  up: string
  root: string
  noMilestone: string
  lost: string
  loading: string
  starting: string
  running: string
  cancel: string
  cancelled: string
  mayContinue: string
  stats: Record<string, string>
  notStarted: string
  /** A start that failed naming a code the panel lists no refusal for (client.ts REFUSAL_CODES): what it means (se-24ey.dcj). */
  notStartedFor: Record<'binding-mismatch' | 'invalid-request', string>
  /** Automatic refresh held after a list read was declined or left unanswered (main.ts holdIfAsked). */
  autoHeld: string
  moaiError: (message: string) => string
  failed: string
  unreadable: string
  refused: Record<RefusalCode, string>
  notInitialized: [string, string]
  shell: ShellCopy
  list: ListCopy
  board: BoardCopy
  detail: DetailCopy
  toolbar: ToolbarCopy
  keys: KeysCopy
  table: KeyTableCopy
  wiki: WikiCopy
}

const en: Copy = {
  hint: "Runs in this project's folder, reading only. Each kind of read asks once per session in a confirmation that shows the machine and folder.",
  refreshedAt: (time) => `Updated ${time}`, empty: 'Nothing here.', more: (n) => `${n} more not shown`,
  up: '..', root: 'All', noMilestone: '(no milestone)', lost: '(lost)', loading: 'Loading…',
  starting: 'Starting… approve the confirmation if it is showing.', running: 'Running on the machine…',
  cancel: 'Cancel', cancelled: 'The run was cancelled.', mayContinue: 'Processes on the machine may not have finished yet.',
  stats: { todo: 'To do', in_progress: 'In progress', review: 'Review', done: 'Done' },
  notStarted: 'The run did not start, and Desktop gave no reason. Click the refresh time to try again; if it keeps happening, restart Desktop.',
  notStartedFor: {
    'binding-mismatch': 'The run did not start: Desktop could not match this panel to its machine and project folder (the selection changed, or the machine reconnected). Open the project conversation again, or restart Desktop, then try again.',
    'invalid-request': 'The run did not start: Desktop did not accept the request. Update the Moai extension.',
  },
  autoHeld: 'Automatic refresh is paused so the confirmation does not come back by itself. Click the refresh time to read again.',
  moaiError: (message) => `Moai reported an error: ${message}`, failed: 'The run failed.', unreadable: 'The run finished, but its result could not be read.',
  refused: {
    declined: 'The run did not start: the confirmation was declined.',
    'approval-timeout': 'The confirmation was not answered in time, so nothing ran. Try again.',
    'workspace-busy': 'Another change is still running in this folder on that machine. Try again when it finishes.',
    'tool-missing': 'Moai is not installed on that machine. Install it for that machine under Machine tools in Settings → Extensions.',
    'unsupported-platform': "Moai cannot run on that machine's operating system.",
    'unsupported-daemon': "That machine's Saycode runtime is too old to run Moai. Update the runtime, then try again.",
  },
  notInitialized: ['This project has no Moai board yet. On that machine, run ', ' once in the project folder, then try again. If moai is not found, run it from the Moai path shown under Machine tools in Settings → Extensions.'],
  shell: SHELL.en, list: LIST.en, board: BOARD.en, detail: DETAIL.en, toolbar: TOOLBAR.en, keys: KEYS.en,
  table: TABLE.en, wiki: WIKI.en,
}

const ko: Copy = {
  hint: '이 프로젝트 폴더에서 읽기만 합니다. 읽는 명령마다 앱을 켜 둔 동안 처음 한 번, 머신과 폴더를 보여주는 확인 창이 뜹니다.',
  refreshedAt: (time) => `${time} 갱신`, empty: '항목이 없습니다.', more: (n) => `그 밖에 ${n}개는 표시하지 않았습니다`,
  up: '..', root: '전체', noMilestone: '(마일스톤 없음)', lost: '(길 잃음)', loading: '불러오는 중…',
  starting: '시작하는 중… 확인 창이 떠 있으면 승인하세요.', running: '머신에서 실행하는 중…',
  cancel: '취소', cancelled: '실행을 취소했습니다.', mayContinue: '머신에서 프로세스가 아직 끝나지 않았을 수 있습니다.',
  stats: { todo: '할 일', in_progress: '진행 중', review: '검토', done: '완료' },
  notStarted: '실행이 시작되지 않았고 Desktop이 이유를 알려 주지 않았습니다. 갱신 시각을 눌러 다시 시도하고, 계속되면 Desktop을 다시 시작하세요.',
  notStartedFor: {
    'binding-mismatch': '실행이 시작되지 않았습니다. Desktop이 이 패널의 머신과 프로젝트 폴더를 확인하지 못했습니다(선택이 바뀌었거나 머신이 다시 연결됨). 프로젝트 대화를 다시 열거나 Desktop을 다시 시작한 뒤 다시 시도하세요.',
    'invalid-request': '실행이 시작되지 않았습니다. Desktop이 요청을 받아들이지 않았습니다. Moai 확장을 업데이트하세요.',
  },
  autoHeld: '확인 창이 저절로 다시 뜨지 않도록 자동 갱신을 멈췄습니다. 갱신 시각을 누르면 다시 읽습니다.',
  moaiError: (message) => `Moai 오류: ${message}`, failed: '실행에 실패했습니다.', unreadable: '실행은 끝났지만 결과를 읽지 못했습니다.',
  refused: {
    declined: '확인 창에서 거부해 실행하지 않았습니다.',
    'approval-timeout': '확인 창에 시간 안에 응답하지 않아 실행하지 않았습니다. 다시 시도하세요.',
    'workspace-busy': '그 머신의 이 폴더에서 다른 변경이 아직 실행 중입니다. 끝난 뒤 다시 시도하세요.',
    'tool-missing': '그 머신에 Moai가 설치되어 있지 않습니다. 설정 → 확장의 머신 도구에서 그 머신에 Moai를 설치하세요.',
    'unsupported-platform': '그 머신의 운영체제에서는 Moai를 실행할 수 없습니다.',
    'unsupported-daemon': '그 머신의 Saycode 런타임이 오래되어 Moai를 실행할 수 없습니다. 런타임을 업데이트한 뒤 다시 시도하세요.',
  },
  notInitialized: ['이 프로젝트에는 아직 Moai 보드가 없습니다. 그 머신의 프로젝트 폴더에서 ', '을 한 번 실행한 뒤 다시 시도하세요. moai를 찾을 수 없으면 설정 → 확장의 머신 도구에 표시된 Moai 경로로 실행하세요.'],
  shell: SHELL.ko, list: LIST.ko, board: BOARD.ko, detail: DETAIL.ko, toolbar: TOOLBAR.ko, keys: KEYS.ko,
  table: TABLE.ko, wiki: WIKI.ko,
}

const ja: Copy = {
  hint: 'このプロジェクトのフォルダで読み取りだけを行います。読み取りの種類ごとに、アプリの起動中に最初の一回、マシンとフォルダを示す確認画面が表示されます。',
  refreshedAt: (time) => `${time} 更新`, empty: '項目はありません。', more: (n) => `ほかに ${n} 件は表示していません`,
  up: '..', root: 'すべて', noMilestone: '(マイルストーンなし)', lost: '(迷子)', loading: '読み込み中…',
  starting: '開始しています… 確認画面が表示されていれば承認してください。', running: 'マシンで実行中…',
  cancel: 'キャンセル', cancelled: '実行をキャンセルしました。', mayContinue: 'マシン上のプロセスはまだ終了していない可能性があります。',
  stats: { todo: '未着手', in_progress: '進行中', review: 'レビュー', done: '完了' },
  notStarted: '実行が開始されず、Desktop から理由が返りませんでした。更新時刻をクリックしてもう一度お試しください。繰り返す場合は Desktop を再起動してください。',
  notStartedFor: {
    'binding-mismatch': '実行が開始されませんでした。Desktop がこのパネルのマシンとプロジェクトフォルダを確認できませんでした（選択が変わったか、マシンが再接続されました）。プロジェクトの会話を開き直すか Desktop を再起動してから、もう一度お試しください。',
    'invalid-request': '実行が開始されませんでした。Desktop が要求を受け付けませんでした。Moai 拡張機能を更新してください。',
  },
  autoHeld: '確認画面が自動で再表示されないよう、自動更新を止めました。更新時刻をクリックすると再読み込みします。',
  moaiError: (message) => `Moai のエラー: ${message}`, failed: '実行に失敗しました。', unreadable: '実行は完了しましたが、結果を読み取れませんでした。',
  refused: {
    declined: '確認画面で拒否されたため、実行しませんでした。',
    'approval-timeout': '確認画面に時間内に応答がなかったため、実行しませんでした。もう一度お試しください。',
    'workspace-busy': 'そのマシンのこのフォルダでは、別の変更がまだ実行中です。終了してからもう一度お試しください。',
    'tool-missing': 'そのマシンに Moai がインストールされていません。設定 → 拡張機能のマシンツールから、そのマシンに Moai をインストールしてください。',
    'unsupported-platform': 'そのマシンの OS では Moai を実行できません。',
    'unsupported-daemon': 'そのマシンの Saycode ランタイムが古いため、Moai を実行できません。ランタイムを更新してから、もう一度お試しください。',
  },
  notInitialized: ['このプロジェクトにはまだ Moai ボードがありません。そのマシンのプロジェクトフォルダで ', ' を一度実行してから、もう一度お試しください。moai が見つからない場合は、設定 → 拡張機能のマシンツールに表示される Moai のパスから実行してください。'],
  shell: SHELL.ja, list: LIST.ja, board: BOARD.ja, detail: DETAIL.ja, toolbar: TOOLBAR.ja, keys: KEYS.ja,
  table: TABLE.ja, wiki: WIKI.ja,
}

const zh: Copy = {
  hint: '在此项目的文件夹中只做读取。每种读取在应用运行期间首次进行时确认一次，确认窗口会显示机器和文件夹。',
  refreshedAt: (time) => `${time} 更新`, empty: '没有项目。', more: (n) => `另有 ${n} 项未显示`,
  up: '..', root: '全部', noMilestone: '(无里程碑)', lost: '(无归属)', loading: '加载中…',
  starting: '正在启动… 如果显示了确认窗口，请批准。', running: '正在机器上运行…',
  cancel: '取消', cancelled: '已取消运行。', mayContinue: '机器上的进程可能尚未结束。',
  stats: { todo: '待办', in_progress: '进行中', review: '审核', done: '完成' },
  notStarted: '运行未开始，Desktop 也未说明原因。请点击刷新时间重试；如果反复出现，请重启 Desktop。',
  notStartedFor: {
    'binding-mismatch': '运行未开始：Desktop 无法确认此面板所属的机器和项目文件夹（选择已更改，或机器已重新连接）。请重新打开项目对话或重启 Desktop 后重试。',
    'invalid-request': '运行未开始：Desktop 不接受此请求。请更新 Moai 扩展。',
  },
  autoHeld: '为避免确认窗口自动再次出现，已暂停自动刷新。点击刷新时间可重新读取。',
  moaiError: (message) => `Moai 错误: ${message}`, failed: '运行失败。', unreadable: '运行已结束，但无法读取结果。',
  refused: {
    declined: '已在确认窗口中拒绝，因此未运行。',
    'approval-timeout': '未在规定时间内回应确认窗口，因此未运行。请重试。',
    'workspace-busy': '该机器的此文件夹中仍有其他更改正在运行。请在其结束后重试。',
    'tool-missing': '该机器上未安装 Moai。请在「设置 → 扩展」的机器工具中为该机器安装 Moai。',
    'unsupported-platform': '该机器的操作系统无法运行 Moai。',
    'unsupported-daemon': '该机器的 Saycode 运行时版本过旧，无法运行 Moai。请更新运行时后重试。',
  },
  notInitialized: ['此项目还没有 Moai 看板。请在该机器的项目文件夹中运行一次 ', '，然后重试。如果找不到 moai，请使用「设置 → 扩展」的机器工具中显示的 Moai 路径运行。'],
  shell: SHELL.zh, list: LIST.zh, board: BOARD.zh, detail: DETAIL.zh, toolbar: TOOLBAR.zh, keys: KEYS.zh,
  table: TABLE.zh, wiki: WIKI.zh,
}

export const COPY: Record<Language, Copy> = { en, ko, ja, zh }

/** What a start that failed says: the words for the code it named when the panel has them, else the general ones. */
export function notStartedText(copy: Copy, code?: string): string {
  return code !== undefined && Object.hasOwn(copy.notStartedFor, code) ? copy.notStartedFor[code as keyof Copy['notStartedFor']] : copy.notStarted
}

/** The host document's language, else the browser's, else English. */
export function copyFor(language: string | undefined): Copy {
  const base = (language || 'en').split('-')[0]!.toLowerCase()
  return COPY[base as Language] ?? en
}
