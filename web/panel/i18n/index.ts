/**
 * The page's words in English, Korean, Japanese and Chinese. The top level is carried over from the 1.x
 * panel; newer words live in one group per area (`shell`, `list`, `board`, `detail`, `toolbar`, `keys`, `table`,
 * `wiki`), each with its own interface and its four languages side by side below, so the stages that build those
 * areas add their words inside their own group without touching another's lines.
 */
import type { VersionCopy } from '../data/about.js'
import type { SortKey } from '../data/query.js'
import type { CellField, HelpGroupId, KeyLabel, LayerId, SortField, StateId } from '../keys/table.js'

export type Language = 'en' | 'ko' | 'ja' | 'zh'

// ── the header: the Version line and the tips (Saycode panel 2.1.0, se-qkga / se-irhr) ─────────────────

/**
 * The header's Version line in moai's own words (its `tui.version.*` keys in i18n/*.json, se-qkga). moai has them in
 * English and Korean only and falls back to English elsewhere, so Japanese and Chinese read the English, as the TUI does.
 */
const VERSION_EN: VersionCopy = {
  newer: '{tag} is out', same: 'latest ({tag})', ahead: 'ahead of the latest release ({tag})', unchecked: 'latest not checked',
  uncheckedFor: {
    offline: 'latest not checked (no network)', timeout: 'latest not checked (timed out)', rate_limited: 'latest not checked (throttled)',
    http: 'latest not checked (HTTP error)', tls: 'latest not checked (TLS failed)', garbled: 'latest not checked (bad answer)',
    odd_tag: 'latest not checked (odd tag)', failed: 'latest not checked (call failed)',
  },
  seenToday: '{tag} seen today', seenDay: '{tag} seen 1 day ago', seenDays: '{tag} seen {d} days ago', seenEarlier: '{tag} seen earlier',
}

const VERSION_KO: VersionCopy = {
  newer: '새 판 {tag}', same: '최신 ({tag})', ahead: '릴리스보다 앞선 판 ({tag})', unchecked: '최신 확인 안 함',
  uncheckedFor: {
    offline: '최신 확인 안 함 (네트워크 없음)', timeout: '최신 확인 안 함 (타임아웃)', rate_limited: '최신 확인 안 함 (요청 한도 초과)',
    http: '최신 확인 안 함 (서버 오류)', tls: '최신 확인 안 함 (TLS 실패)', garbled: '최신 확인 안 함 (응답을 못 읽음)',
    odd_tag: '최신 확인 안 함 (태그 꼴이 아님)', failed: '최신 확인 안 함 (호출 실패)',
  },
  seenToday: '오늘 본 판 {tag}', seenDay: '1일 전에 본 판 {tag}', seenDays: '{d}일 전에 본 판 {tag}', seenEarlier: '전에 본 판 {tag}',
}

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

const TIPS_KO: TipsCopy = {
  title: '에이전트에게 이렇게 말해 보세요',
  intro: 'Claude Code·Codex·Saycode 세션에 그대로 붙여 넣거나 조금 고쳐서 쓰세요. moai 스킬이 깔린 저장소에서 통합니다.',
  openAll: '팁 모두 보기',
  copy: '복사', copied: '복사함', close: '닫기',
  groups: [
    { title: '시작하기', tips: [
      '뭐부터 하면 될까?',
      'se-43oh 시작해줘 (또는 43oh 시작해줘)',
      '진행 상황 알려줘',
      '이거 나중에 하자. 백로그에 넣어줘',
    ] },
    { title: '계획과 이슈', tips: [
      '유저관리설정에서 유저 검색 창을 만들어야 한다. `opus5.x xhigh` 로 계획을 세워줘',
      '이 기능 요청을 에픽 하나와 이슈 3~7개로 나눠서 먼저 보여줘',
      'se-43oh 아래에 리뷰 이슈를 만들고 리뷰를 진행해줘',
    ] },
    { title: '마일스톤', tips: [
      'v1.22.x의 마일스톤을 넣고 1.22에서 리뷰에서 발견된 버그 백로그를 마일스톤에 포함시켜줘.',
      '마일스톤 v0.1.2 를 시작하고 리뷰때 버그가 아닌 백로그는 만들지말 것',
      'v0.1.2 마일스톤에 남은 일과 막힌 일을 정리해줘',
    ] },
    { title: '백로그 정리', tips: [
      '백로그를 유사한 것들 끼리는 함께 작업할 수 있게 합쳐주고, 필요없다 판단되는 것은 삭제해줘.',
      '오래된 백로그 중 아직 유효한 것만 이슈로 올려줘',
    ] },
    { title: '감독과 모델', tips: [
      '이 세션은 moai 감독 세션이다. 세이코드 세션 3개를 만들고 서브에이전트로 작업을 시작할 것.',
      '작업은 sonnet5.x medium 으로 진행하고 리뷰를 opus5.5 high 또는 난이도에 따라 xhigh 로 할 것',
      '놀고 있는 세션에 백로그를 나눠 줘',
    ] },
    { title: '복구', tips: [
      '작업중에 서버가 죽었다. 작업목록을 확인하여 이전 세션을 복구하고 작업을 계속 진행해줘.',
      '/moai:moai-recover 로 죽은 세션을 되살려줘',
    ] },
    { title: '위키', tips: [
      '/moai:moai-wiki 또는 모아이 위키 스킬을 사용하여 프로그램 전체의 위키 문서를 작성하고 항상 위키를 업데이트할 것',
      '이번 에픽에서 바뀐 것을 위키에 반영해줘',
    ] },
  ],
}

const TIPS_EN: TipsCopy = {
  title: 'Things to say to an agent',
  intro: 'Paste one into a Claude Code, Codex or Saycode session as it is, or change it to fit. They work in a repository with moai\'s skills.',
  openAll: 'All tips',
  copy: 'Copy', copied: 'Copied', close: 'Close',
  groups: [
    { title: 'Getting going', tips: [
      'What should I do first?',
      'Start se-43oh (or: start 43oh)',
      'How is it going?',
      'Let us do this later. Put it in the backlog',
    ] },
    { title: 'Plans and issues', tips: [
      'User management settings need a user search box. Plan it with `opus5.x xhigh`',
      'Split this feature request into one epic and 3 to 7 issues, and show me first',
      'Make a review issue under se-43oh and run the review',
    ] },
    { title: 'Milestones', tips: [
      'Add the v1.22.x milestone and put the bug backlog the 1.22 review found into it.',
      'Start milestone v0.1.2, and make no backlog items in reviews except bugs',
      'Sum up what is left and what is blocked in milestone v0.1.2',
    ] },
    { title: 'Backlog grooming', tips: [
      'Merge backlog items that are alike so they can be worked on together, and delete the ones you judge unneeded.',
      'Raise only the old backlog items that still hold into issues',
    ] },
    { title: 'Supervising and models', tips: [
      'This session is the moai supervisor. Make three Saycode sessions and start the work with subagents.',
      'Work with sonnet5.x medium, and review with opus5.5 high, or xhigh when it is hard',
      'Hand the backlog out to the idle sessions',
    ] },
    { title: 'Recovering', tips: [
      'The server died mid-work. Check the work list, bring the earlier sessions back and carry on.',
      'Bring the dead sessions back with /moai:moai-recover',
    ] },
    { title: 'Wiki', tips: [
      'With /moai:moai-wiki (the moai wiki skill), write the wiki for the whole program and keep it up to date.',
      'Bring the wiki up to date with what this epic changed',
    ] },
  ],
}

// ── shell: tab bar, toggles, path line, banners (E2 se-qwkz.2v9) ────────────────────────────────────

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
  /** The archive read failed (`SPC v o`); why follows. The board's rows stand. */
  archiveFailed: string
  /** The header's Version line (view/brand.ts). */
  version: VersionCopy
  /** The header's tips and the tips page (view/tips.ts). */
  tips: TipsCopy
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
    sortFields: { priority: 'priority', created: 'created', updated: 'updated', status: 'status', assignee: 'assignee', title: 'title', due: 'due', id: 'id' },
    reversed: '(reversed)',
    clearHint: 'Esc clears it',
    wikiPlaceholder: 'The wiki is not here yet.',
    wikiUnavailable: 'The wiki cannot be read here yet.',
    wikiListFailed: (reason) => `The wiki pages could not be read: ${reason}`,
    archiveFailed: 'Archived rows could not be read:',
    version: VERSION_EN,
    tips: TIPS_EN,
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
    sortFields: { priority: '우선순위', created: '만든 날', updated: '고친 날', status: '칸', assignee: '담당', title: '제목', due: '기한', id: 'ID' },
    reversed: '(뒤집음)',
    clearHint: 'Esc로 해제',
    wikiPlaceholder: '위키는 아직 준비 중입니다.',
    wikiUnavailable: '아직 여기서는 위키를 읽을 수 없습니다.',
    wikiListFailed: (reason) => `위키 페이지 목록을 읽지 못했습니다: ${reason}`,
    archiveFailed: '아카이브된 항목을 읽지 못했습니다:',
    version: VERSION_KO,
    tips: TIPS_KO,
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
    sortFields: { priority: '優先度', created: '作成', updated: '更新', status: '列', assignee: '担当', title: 'タイトル', due: '期限', id: 'ID' },
    reversed: '(逆順)',
    clearHint: 'Esc で解除',
    wikiPlaceholder: 'Wiki はまだ準備中です。',
    wikiUnavailable: 'ここではまだ Wiki を読めません。',
    wikiListFailed: (reason) => `Wiki のページ一覧を読めませんでした: ${reason}`,
    archiveFailed: 'アーカイブ済みの項目を読めませんでした:',
    version: VERSION_EN,
    tips: TIPS_EN,
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
    sortFields: { priority: '优先级', created: '创建', updated: '更新', status: '列', assignee: '负责人', title: '标题', due: '期限', id: 'ID' },
    reversed: '(反向)',
    clearHint: '按 Esc 清除',
    wikiPlaceholder: 'Wiki 尚未就绪。',
    wikiUnavailable: '这里暂时无法读取 Wiki。',
    wikiListFailed: (reason) => `无法读取 Wiki 页面列表: ${reason}`,
    archiveFailed: '无法读取已归档的条目:',
    version: VERSION_EN,
    tips: TIPS_EN,
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
    search: 'Search', searchPlaceholder: 'Search (/)',
    filter: 'Filter', filterPlaceholder: 'status=todo tag=bug,ui (Enter applies)', removeChip: 'Remove filter item', hints: 'Suggestions',
    sort: 'Sort', sortEdit: 'Edit sort', reversed: 'reversed', replace: 'Only this', append: 'Add', flip: 'Flip direction',
    sortKeys: { priority: 'Priority', created: 'Created', updated: 'Updated', status: 'Status', assignee: 'Assignee', title: 'Title', due: 'Due', id: 'ID' },
    hidden: (n) => `${n} hidden`,
    hiddenReasons: { column: 'hidden column', deferred: 'deferred', backlog: 'backlog', archived: 'archived' },
    sortDefaultSessionNote: 'D: save as the default order — this browser keeps no site data, so it lasts until the tab is closed.',
  },
  ko: {
    search: '검색', searchPlaceholder: '검색 (/)',
    filter: '필터', filterPlaceholder: 'status=todo tag=bug,ui (Enter로 적용)', removeChip: '필터 항목 지우기', hints: '자동 완성',
    sort: '정렬', sortEdit: '정렬 편집', reversed: '뒤집힘', replace: '이것만', append: '추가', flip: '방향 바꿈',
    sortKeys: { priority: '우선순위', created: '만든 때', updated: '고친 때', status: '상태', assignee: '담당', title: '제목', due: '기한', id: 'ID' },
    hidden: (n) => `${n}건 숨김`,
    hiddenReasons: { column: '숨긴 칸', deferred: '미룸', backlog: '백로그', archived: '보관' },
    sortDefaultSessionNote: 'D: 기본 정렬로 저장 — 이 브라우저가 사이트 데이터를 안 남겨 탭을 닫을 때까지만 유지됩니다.',
  },
  ja: {
    search: '検索', searchPlaceholder: '検索 (/)',
    filter: 'フィルター', filterPlaceholder: 'status=todo tag=bug,ui (Enter で適用)', removeChip: 'フィルター項目を削除', hints: '候補',
    sort: '並べ替え', sortEdit: '並べ替えを編集', reversed: '逆順', replace: 'これだけ', append: '追加', flip: '向きを反転',
    sortKeys: { priority: '優先度', created: '作成', updated: '更新', status: '状態', assignee: '担当', title: 'タイトル', due: '期限', id: 'ID' },
    hidden: (n) => `${n} 件を非表示`,
    hiddenReasons: { column: '非表示の列', deferred: '保留', backlog: 'バックログ', archived: 'アーカイブ' },
    sortDefaultSessionNote: 'D: 既定の並べ替えとして保存 — このブラウザはサイトのデータを残さないため、タブを閉じるまで保持されます。',
  },
  zh: {
    search: '搜索', searchPlaceholder: '搜索 (/)',
    filter: '筛选', filterPlaceholder: 'status=todo tag=bug,ui (按 Enter 应用)', removeChip: '删除筛选项', hints: '建议',
    sort: '排序', sortEdit: '编辑排序', reversed: '反向', replace: '仅此项', append: '添加', flip: '反转方向',
    sortKeys: { priority: '优先级', created: '创建', updated: '更新', status: '状态', assignee: '负责人', title: '标题', due: '期限', id: 'ID' },
    hidden: (n) => `已隐藏 ${n} 项`,
    hiddenReasons: { column: '隐藏的列', deferred: '推迟', backlog: '待办池', archived: '已归档' },
    sortDefaultSessionNote: 'D: 保存为默认排序 — 此浏览器不保留网站数据，关闭标签页前有效。',
  },
}

// ── keys: the key router's words (se-jwl3); the key table's own labels are the `table` group below ───────

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
  unlisted: (now: string) => string
}

const KEYS: Record<Language, KeysCopy> = {
  en: {
    focusHint: 'Click the page once and the keys work',
    sortDefaultSaved: 'Saved as the default order.',
    sortDefaultSavedSession: 'Saved as the default order. This browser keeps no site data, so it lasts until the tab is closed.',
    readUnavailable: 'Marking items read is not available on this page.',
    readNoGroup: 'Move to a milestone or epic to mark its items read.',
    zone: {
      title: 'Timezone', filter: 'Type to filter', now: 'now', browser: 'Browser zone ({zone})', tui: 'Explorer zone ({zone})',
      noMatch: 'No name on this list matches.',
      unlisted: (now) => `${now} in use is not on this list. Enter alone keeps it; move or type to pick one.`,
    },
  },
  ko: {
    focusHint: '페이지를 한 번 누르면 키가 듣습니다',
    sortDefaultSaved: '기본 정렬로 저장했습니다.',
    sortDefaultSavedSession: '기본 정렬로 저장했습니다. 이 브라우저가 사이트 데이터를 안 남겨 탭을 닫을 때까지만 유지됩니다.',
    readUnavailable: '이 페이지에서는 읽음 표시를 쓸 수 없습니다.',
    readNoGroup: '읽음으로 표시할 마일스톤이나 에픽으로 옮겨 가세요.',
    zone: {
      title: '시간대', filter: '쳐서 거르기', now: '지금', browser: '브라우저 시간대 ({zone})', tui: '탐색기 시간대 ({zone})',
      noMatch: '맞는 이름이 없습니다.',
      unlisted: (now) => `지금 쓰는 ${now} 는 목록에 없습니다. 그냥 Enter 는 그대로 둡니다. 옮기거나 쳐서 고르세요.`,
    },
  },
  ja: {
    focusHint: 'ページを一度クリックするとキーが効きます',
    sortDefaultSaved: '既定の並べ替えとして保存しました。',
    sortDefaultSavedSession: '既定の並べ替えとして保存しました。このブラウザはサイトのデータを残さないため、タブを閉じるまで保持されます。',
    readUnavailable: 'このページでは既読にできません。',
    readNoGroup: '既読にするマイルストーンかエピックに移動してください。',
    zone: {
      title: 'タイムゾーン', filter: '入力して絞り込む', now: '使用中', browser: 'ブラウザのタイムゾーン ({zone})', tui: 'エクスプローラーのタイムゾーン ({zone})',
      noMatch: '一致する名前はありません。',
      unlisted: (now) => `使用中の ${now} は一覧にありません。Enter だけならそのまま、移動か入力で選びます。`,
    },
  },
  zh: {
    focusHint: '点击页面一次即可使用按键',
    sortDefaultSaved: '已保存为默认排序。',
    sortDefaultSavedSession: '已保存为默认排序。此浏览器不保留网站数据，关闭标签页前有效。',
    readUnavailable: '此页面不能标记为已读。',
    readNoGroup: '请移到要标记为已读的里程碑或史诗。',
    zone: {
      title: '时区', filter: '输入以筛选', now: '当前', browser: '浏览器时区 ({zone})', tui: 'moai tui 时区 ({zone})',
      noMatch: '没有匹配的名称。',
      unlisted: (now) => `当前的 ${now} 不在列表中。直接按 Enter 保持不变，移动或输入以选择。`,
    },
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
    'view.deferred': 'Deferred', 'view.backlog': 'Backlog', 'view.archived': 'Archive', 'view.worktree': 'Worktrees',
    'view.all': 'All but archived', 'view.allHelp': 'Show all (archive stays hidden)',
    'view.detail': 'Details', 'view.raw': 'Raw', 'view.rawHelp': 'Raw or rendered', 'sort.edit': 'Edit the order…',
    'read.all': 'Mark all read', 'read.group': 'Group read', 'read.groupHelp': 'Mark the group read',
    'option.detailAt': 'Details at', 'option.detailAtHelp': 'Details position (left ↔ bottom)',
    'option.timezone': 'Timezone', 'option.timezoneHelp': 'Timezone of the dates (pick one)', 'project.jumpHelp': 'Go to project 1–9',
    'menu.close': 'Close', 'menu.up': 'Up one level',
    'prompt.apply': 'Apply (with filter values listed, insert one)', 'prompt.cancel': 'Cancel, back to the cursor before',
    'prompt.next': 'Complete the filter', 'prompt.prev': 'Complete the filter, back',
    'prompt.up': 'Previous filter value', 'prompt.down': 'Next filter value', 'prompt.clear': 'Clear the whole input', 'prompt.deleteWord': 'Delete a word',
    'sorter.down': 'Down', 'sorter.up': 'Up', 'sorter.lower': 'Move later', 'sorter.raise': 'Move earlier', 'sorter.toggle': 'Use or drop',
    'sorter.flip': 'Flip direction', 'sorter.apply': 'Apply', 'sorter.default': 'Save as default', 'sorter.cancel': 'Cancel',
    'wiki.enter': 'Open the page / pick or follow a link', 'wiki.back': 'Go back the way you came', 'wiki.page': 'From the list to the page',
    'wiki.nextLink': 'Next link', 'wiki.prevLink': 'Previous link', 'wiki.search': 'Search titles and text', 'wiki.clear': 'Drop the link, then the search',
  },
  root: 'Menu',
  layers: { g: 'Go', p: 'Project', v: 'View', s: 'Sort', c: 'Columns', m: 'Read', o: 'Options', w: 'Window' },
  groups: {
    move: 'Move', enter: 'In, out and expand', wiki: 'Wiki', focus: 'Pane focus', tab: 'Switch views', project: 'Projects', search: 'Search and filter', read: 'Read',
    view: 'View (SPC v)', sort: 'Sort (SPC s)', cell: 'Columns (SPC c)', option: 'Options (SPC o)', other: 'Other',
    menu: 'Inside the SPC menu', prompt: 'Search and filter input', sorter: 'Sort editor',
  },
  states: {
    shown: '[shown]', hidden: '[hidden]', asc: '[↑ asc]', desc: '[↓ desc]', left: '[left]', bottom: '[bottom]',
    raw: '[raw]', rendered: '[rendered]', list: '[list]', board: '[board]', wiki: '[wiki]',
  },
  sortFields: { priority: 'Priority', created: 'Created', updated: 'Updated', status: 'Status', assignee: 'Assignee', title: 'Title', due: 'Due' },
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
    'view.deferred': '미룸', 'view.backlog': '백로그', 'view.archived': '아카이브', 'view.worktree': '워크트리',
    'view.all': '아카이브 빼고 모두', 'view.allHelp': '모두 보이기 (아카이브 제외)',
    'view.detail': '상세', 'view.raw': '원문', 'view.rawHelp': '원문 또는 렌더링', 'sort.edit': '정렬 편집…',
    'read.all': '모두 읽음', 'read.group': '그룹 읽음', 'read.groupHelp': '그룹을 읽음으로',
    'option.detailAt': '상세 위치', 'option.detailAtHelp': '상세 위치 (왼쪽 ↔ 아래)',
    'option.timezone': '시간대', 'option.timezoneHelp': '날짜의 시간대 (고르기)', 'project.jumpHelp': '프로젝트 1~9 로 가기',
    'menu.close': '닫기', 'menu.up': '한 층 위로',
    'prompt.apply': '적용 (필터 값 목록이 있으면 값 넣기)', 'prompt.cancel': '취소하고 열기 전 커서로',
    'prompt.next': '필터 자동완성', 'prompt.prev': '필터 자동완성 거꾸로',
    'prompt.up': '필터 값 후보 위', 'prompt.down': '필터 값 후보 아래', 'prompt.clear': '입력 전체 지움', 'prompt.deleteWord': '단어 지움',
    'sorter.down': '아래로', 'sorter.up': '위로', 'sorter.lower': '순서 뒤로', 'sorter.raise': '순서 앞으로', 'sorter.toggle': '넣기·빼기',
    'sorter.flip': '방향 반전', 'sorter.apply': '적용', 'sorter.default': '기본값으로 저장', 'sorter.cancel': '취소',
    'wiki.enter': '페이지 열기 / 링크 선택·이동', 'wiki.back': '건너온 길 되감기', 'wiki.page': '목록에서 본문으로',
    'wiki.nextLink': '다음 링크', 'wiki.prevLink': '이전 링크', 'wiki.search': '제목·본문 검색', 'wiki.clear': '링크 선택 해제 → 검색 해제',
  },
  root: '메뉴',
  layers: { g: '이동', p: '프로젝트', v: '보기', s: '정렬', c: '열', m: '읽음', o: '옵션', w: '창' },
  groups: {
    move: '이동', enter: '드나들기·펼침', wiki: '위키', focus: '패널 포커스', tab: '화면 전환', project: '프로젝트', search: '검색·필터', read: '읽음',
    view: '보기 (SPC v)', sort: '정렬 (SPC s)', cell: '열 (SPC c)', option: '옵션 (SPC o)', other: '기타',
    menu: 'SPC 메뉴 안에서', prompt: '검색·필터 입력', sorter: '정렬 편집',
  },
  states: {
    shown: '[보임]', hidden: '[숨김]', asc: '[↑ 오름]', desc: '[↓ 내림]', left: '[왼쪽]', bottom: '[아래]',
    raw: '[원문]', rendered: '[렌더링]', list: '[목록]', board: '[칸반]', wiki: '[위키]',
  },
  sortFields: { priority: '우선순위', created: '생성일', updated: '수정일', status: '상태', assignee: '담당자', title: '제목', due: '기한' },
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
    'view.deferred': '保留', 'view.backlog': 'バックログ', 'view.archived': 'アーカイブ', 'view.worktree': 'ワークツリー',
    'view.all': 'アーカイブ以外', 'view.allHelp': 'すべて表示 (アーカイブは除く)',
    'view.detail': '詳細', 'view.raw': '原文', 'view.rawHelp': '原文またはレンダリング', 'sort.edit': '並べ替えを編集…',
    'read.all': 'すべて既読', 'read.group': 'グループ既読', 'read.groupHelp': 'グループを既読に',
    'option.detailAt': '詳細の位置', 'option.detailAtHelp': '詳細の位置 (左 ↔ 下)',
    'option.timezone': 'タイムゾーン', 'option.timezoneHelp': '日時のタイムゾーン (選ぶ)', 'project.jumpHelp': 'プロジェクト 1〜9 へ移動',
    'menu.close': '閉じる', 'menu.up': '一つ上の階層へ',
    'prompt.apply': '適用 (フィルター値の候補があれば挿入)', 'prompt.cancel': 'キャンセルして開く前のカーソルへ',
    'prompt.next': 'フィルターを補完', 'prompt.prev': 'フィルターを逆に補完',
    'prompt.up': 'フィルター値の前の候補', 'prompt.down': 'フィルター値の次の候補', 'prompt.clear': '入力をすべて消去', 'prompt.deleteWord': '単語を削除',
    'sorter.down': '下へ', 'sorter.up': '上へ', 'sorter.lower': '順序を後ろへ', 'sorter.raise': '順序を前へ', 'sorter.toggle': '入れる・外す',
    'sorter.flip': '向きを反転', 'sorter.apply': '適用', 'sorter.default': '既定として保存', 'sorter.cancel': 'キャンセル',
    'wiki.enter': 'ページを開く / リンクを選択・移動', 'wiki.back': '来た道を戻る', 'wiki.page': '一覧から本文へ',
    'wiki.nextLink': '次のリンク', 'wiki.prevLink': '前のリンク', 'wiki.search': 'タイトル・本文を検索', 'wiki.clear': 'リンクの選択を解除 → 検索を解除',
  },
  root: 'メニュー',
  layers: { g: '移動', p: 'プロジェクト', v: '表示', s: '並べ替え', c: '列', m: '既読', o: 'オプション', w: 'ウィンドウ' },
  groups: {
    move: '移動', enter: '出入り・展開', wiki: 'Wiki', focus: 'ペインのフォーカス', tab: '画面の切り替え', project: 'プロジェクト', search: '検索・フィルター', read: '既読',
    view: '表示 (SPC v)', sort: '並べ替え (SPC s)', cell: '列 (SPC c)', option: 'オプション (SPC o)', other: 'その他',
    menu: 'SPC メニューの中', prompt: '検索・フィルターの入力', sorter: '並べ替えの編集',
  },
  states: {
    shown: '[表示]', hidden: '[非表示]', asc: '[↑ 昇順]', desc: '[↓ 降順]', left: '[左]', bottom: '[下]',
    raw: '[原文]', rendered: '[レンダリング]', list: '[一覧]', board: '[カンバン]', wiki: '[Wiki]',
  },
  sortFields: { priority: '優先度', created: '作成日', updated: '更新日', status: '状態', assignee: '担当者', title: 'タイトル', due: '期限' },
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
    'view.deferred': '推迟', 'view.backlog': '待办池', 'view.archived': '归档', 'view.worktree': '工作树',
    'view.all': '除归档外全部', 'view.allHelp': '全部显示 (归档除外)',
    'view.detail': '详情', 'view.raw': '原文', 'view.rawHelp': '原文或渲染', 'sort.edit': '编辑排序…',
    'read.all': '全部标为已读', 'read.group': '分组已读', 'read.groupHelp': '分组标为已读',
    'option.detailAt': '详情位置', 'option.detailAtHelp': '详情位置 (左 ↔ 下)',
    'option.timezone': '时区', 'option.timezoneHelp': '日期的时区 (选择)', 'project.jumpHelp': '切换到项目 1–9',
    'menu.close': '关闭', 'menu.up': '返回上一层',
    'prompt.apply': '应用 (有筛选值候选时插入)', 'prompt.cancel': '取消并回到打开前的光标',
    'prompt.next': '补全筛选', 'prompt.prev': '反向补全筛选',
    'prompt.up': '上一个筛选值候选', 'prompt.down': '下一个筛选值候选', 'prompt.clear': '清空全部输入', 'prompt.deleteWord': '删除一个词',
    'sorter.down': '向下', 'sorter.up': '向上', 'sorter.lower': '顺序后移', 'sorter.raise': '顺序前移', 'sorter.toggle': '加入·移除',
    'sorter.flip': '反转方向', 'sorter.apply': '应用', 'sorter.default': '保存为默认', 'sorter.cancel': '取消',
    'wiki.enter': '打开页面 / 选择或跟随链接', 'wiki.back': '沿来路返回', 'wiki.page': '从列表到正文',
    'wiki.nextLink': '下一个链接', 'wiki.prevLink': '上一个链接', 'wiki.search': '搜索标题和正文', 'wiki.clear': '取消链接选择 → 清除搜索',
  },
  root: '菜单',
  layers: { g: '跳转', p: '项目', v: '视图', s: '排序', c: '列', m: '已读', o: '选项', w: '窗口' },
  groups: {
    move: '移动', enter: '进出·展开', wiki: 'Wiki', focus: '窗格焦点', tab: '切换视图', project: '项目', search: '搜索·筛选', read: '已读',
    view: '视图 (SPC v)', sort: '排序 (SPC s)', cell: '列 (SPC c)', option: '选项 (SPC o)', other: '其他',
    menu: 'SPC 菜单内', prompt: '搜索·筛选输入', sorter: '编辑排序',
  },
  states: {
    shown: '[显示]', hidden: '[隐藏]', asc: '[↑ 升序]', desc: '[↓ 降序]', left: '[左]', bottom: '[下]',
    raw: '[原文]', rendered: '[渲染]', list: '[列表]', board: '[看板]', wiki: '[Wiki]',
  },
  sortFields: { priority: '优先级', created: '创建日期', updated: '更新日期', status: '状态', assignee: '负责人', title: '标题', due: '期限' },
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

// ── the top-level words, and the groups put together ─────────────────────────────────────────────────

export interface Copy {
  hint: string
  refreshedAt: (time: string) => string
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
  moaiError: (message: string) => string
  failed: string
  unreadable: string
  /** Before the lines moai said on stderr beside an answer (client.ts Outcome `notes`). */
  said: string
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
  hint: 'Reading this project through moai web.',
  refreshedAt: (time) => `Updated ${time}`, empty: 'Nothing here.',
  up: '..', root: 'All', noMilestone: '(no milestone)', lost: '(lost)', loading: 'Loading…',
  running: 'Reading…', cancel: 'Cancel', cancelled: 'The read was cancelled.',
  stats: { todo: 'To do', in_progress: 'In progress', review: 'Review', done: 'Done' },
  unreachable: 'moai web cannot be reached — check that it is still running, then click the refresh time.',
  backingOff: 'Reads keep failing — trying again every 10 s instead of every 2 s. Click the refresh time to try now.',
  moaiError: (message) => `Moai reported an error: ${message}`, failed: 'The read failed.', unreadable: 'The answer could not be read.',
  said: 'moai said:',
  shell: SHELL.en, list: LIST.en, board: BOARD.en, detail: DETAIL.en, toolbar: TOOLBAR.en, keys: KEYS.en,
  table: TABLE.en, wiki: WIKI.en,
}

const ko: Copy = {
  hint: 'moai web 으로 이 프로젝트를 읽는 중입니다.',
  refreshedAt: (time) => `${time} 갱신`, empty: '항목이 없습니다.',
  up: '..', root: '전체', noMilestone: '(마일스톤 없음)', lost: '(길 잃음)', loading: '불러오는 중…',
  running: '읽는 중…', cancel: '취소', cancelled: '읽기를 취소했습니다.',
  stats: { todo: '할 일', in_progress: '진행 중', review: '리뷰', done: '완료' },
  unreachable: 'moai web 에 닿지 않습니다 — 아직 도는지 본 뒤 갱신 시각을 누르세요.',
  backingOff: '읽기가 계속 실패합니다 — 2초 대신 10초마다 다시 시도합니다. 지금 시도하려면 갱신 시각을 누르세요.',
  moaiError: (message) => `Moai 오류: ${message}`, failed: '읽지 못했습니다.', unreadable: '답을 읽지 못했습니다.',
  said: 'moai 가 한 말:',
  shell: SHELL.ko, list: LIST.ko, board: BOARD.ko, detail: DETAIL.ko, toolbar: TOOLBAR.ko, keys: KEYS.ko,
  table: TABLE.ko, wiki: WIKI.ko,
}

const ja: Copy = {
  hint: 'moai web でこのプロジェクトを読み込んでいます。',
  refreshedAt: (time) => `${time} 更新`, empty: '項目はありません。',
  up: '..', root: 'すべて', noMilestone: '(マイルストーンなし)', lost: '(迷子)', loading: '読み込み中…',
  running: '読み込み中…', cancel: 'キャンセル', cancelled: '読み込みをキャンセルしました。',
  stats: { todo: '未着手', in_progress: '進行中', review: 'レビュー', done: '完了' },
  unreachable: 'moai web に接続できません — まだ動いているか確認してから、更新時刻をクリックしてください。',
  backingOff: '読み取りが失敗し続けています — 2 秒ではなく 10 秒ごとに再試行します。今すぐ試すには更新時刻をクリックしてください。',
  moaiError: (message) => `Moai のエラー: ${message}`, failed: '読み込みに失敗しました。', unreadable: '応答を読み取れませんでした。',
  said: 'moai のメッセージ:',
  shell: SHELL.ja, list: LIST.ja, board: BOARD.ja, detail: DETAIL.ja, toolbar: TOOLBAR.ja, keys: KEYS.ja,
  table: TABLE.ja, wiki: WIKI.ja,
}

const zh: Copy = {
  hint: '正在通过 moai web 读取此项目。',
  refreshedAt: (time) => `${time} 更新`, empty: '没有项目。',
  up: '..', root: '全部', noMilestone: '(无里程碑)', lost: '(无归属)', loading: '加载中…',
  running: '正在读取…', cancel: '取消', cancelled: '已取消读取。',
  stats: { todo: '待办', in_progress: '进行中', review: '审核', done: '完成' },
  unreachable: '无法连接 moai web — 请确认它仍在运行，然后点击刷新时间。',
  backingOff: '读取持续失败 — 改为每 10 秒重试一次，而不是每 2 秒。点击刷新时间可立即重试。',
  moaiError: (message) => `Moai 错误: ${message}`, failed: '读取失败。', unreadable: '无法读取应答。',
  said: 'moai 提示:',
  shell: SHELL.zh, list: LIST.zh, board: BOARD.zh, detail: DETAIL.zh, toolbar: TOOLBAR.zh, keys: KEYS.zh,
  table: TABLE.zh, wiki: WIKI.zh,
}

export const COPY: Record<Language, Copy> = { en, ko, ja, zh }

/** The words for a language tag (`/api/boot`'s `lang`); a language the page does not carry is English. */
export function copyFor(language: string | undefined): Copy {
  const base = (language || 'en').split('-')[0]!.toLowerCase()
  return COPY[base as Language] ?? en
}
