// Moai panel shell: design tokens, shared components, tab strip and list/detail split.
// These modules are standalone ES modules (panel/view/*.ts, panel/style/*.css); Node strips the types.
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { test } from 'node:test'
import { JSDOM } from 'jsdom'

const panel = new URL('../packages/moai/panel/', import.meta.url)
const { ICONS, iconSvg } = await import(new URL('view/icons.ts', panel))
const components = await import(new URL('view/components.ts', panel))
const tabsModule = await import(new URL('view/tabs.ts', panel))
const splitModule = await import(new URL('view/split.ts', panel))
const responsive = await import(new URL('view/responsive.ts', panel))
const tokensCss = await readFile(new URL('style/tokens.css', panel), 'utf8')
const componentsCss = await readFile(new URL('style/components.css', panel), 'utf8')

const dom = new JSDOM('<!doctype html><body></body>', { pretendToBeVisual: true })
const doc = dom.window.document
const press = (node, key, init = {}) => {
  const event = new dom.window.KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...init })
  node.dispatchEvent(event)
  return event
}

test('tokens: every host token is mapped, with fallbacks and dark overrides', () => {
  for (const name of ['bg', 'surface', 'text-muted', 'text', 'border', 'accent-contrast', 'accent', 'danger', 'radius', 'font']) {
    assert.match(tokensCss, new RegExp(`var\\(--saycode-${name}\\b`), `--saycode-${name} is read`)
  }
  for (const derived of ['hover', 'selected', 'muted-bg', 'accent-hover', 'danger-soft', 'ok', 'ok-soft', 'warn', 'warn-soft', 'border-soft', 'shadow-md']) {
    assert.match(tokensCss, new RegExp(`--${derived}:`), `--${derived} is defined`)
  }
  // The dark theme is reachable by the host attribute and by the OS scheme when the host says nothing.
  assert.match(tokensCss, /:root\[data-theme="dark"\]/)
  assert.match(tokensCss, /prefers-color-scheme: dark\)\s*\{\s*:root:not\(\[data-theme="light"\]\)/)
  assert.match(tokensCss, /#13161A/)
})

test('components css: no gradient, blur or glow; shadows only on menus and tooltips', () => {
  assert.doesNotMatch(componentsCss, /gradient|blur\(|drop-shadow|text-shadow/)
  const rules = [...componentsCss.matchAll(/([^{}]+)\{([^{}]*)\}/g)]
  const shadowed = rules.filter(([, , body]) => /box-shadow:\s*(?!none\b)[^;\s]/.test(body)).map(([, selector]) => selector.replace(/\/\*[^]*?\*\//g, '').trim())
  assert.deepEqual(shadowed, ['.m-menu', '.m-tooltip'])
  assert.doesNotMatch(componentsCss + tokensCss, /@import|url\(/, 'CSP: nothing external')
})

test('icons: every icon the panel needs is inline svg markup', () => {
  for (const name of ['kanban', 'list-tree', 'book-open', 'panel-left', 'panel-bottom', 'search', 'filter', 'arrow-up-down', 'eye', 'columns-3', 'refresh-cw', 'circle-help', 'chevron-right', 'x', 'folder', 'corner-left-up']) {
    assert.ok(ICONS[name], name)
    assert.match(iconSvg(name, 15), /^<svg [^>]*width="15"[^>]*stroke="currentColor"[^>]*>.+<\/svg>$/)
  }
  assert.doesNotMatch(Object.values(ICONS).join(''), /<script|href=|on\w+=/i)
})

test('components: icon button names itself and reflects pressed', () => {
  const button = components.iconButton(doc, { icon: 'panel-left', label: '상세를 왼쪽으로', pressed: true })
  assert.equal(button.getAttribute('aria-label'), '상세를 왼쪽으로')
  assert.equal(button.title, '상세를 왼쪽으로')
  assert.equal(button.getAttribute('aria-pressed'), 'true')
  assert.ok(button.querySelector('svg.m-icon'))
})

test('components: a text button with an icon keeps its name when a narrow toolbar hides the label', () => {
  const folded = components.button(doc, { label: '정렬', icon: 'arrow-up-down' })
  assert.equal(folded.getAttribute('aria-label'), '정렬')
  assert.equal(folded.title, '정렬')
  assert.equal(components.button(doc, { label: '확인' }).hasAttribute('title'), false)
  // The narrow toolbar folds button labels only; the search scope select stays.
  const narrow = componentsCss.match(/@container m-toolbar \([^)]*\) \{([^}]*)\}/)
  assert.ok(narrow)
  assert.match(narrow[1], /\.m-btn-label/)
  assert.doesNotMatch(narrow[1], /m-select/)
})

test('components: text goes in as text, never as markup', () => {
  const hostile = '<img src=x onerror=alert(1)>'
  for (const node of [
    components.chip(doc, { label: hostile }),
    components.emptyState(doc, { title: hostile, hint: hostile }),
    components.banner(doc, 'danger', hostile),
    components.button(doc, { label: hostile }),
    components.menu(doc, [{ id: 'a', label: hostile }], () => {}),
  ]) {
    assert.equal(node.querySelector('img'), null)
    assert.ok(node.textContent.includes(hostile))
  }
})

test('components: chip, banner, search, select and menu behave', () => {
  let removed = 0
  const filter = components.chip(doc, { label: 'tag=x', removeLabel: '필터 지우기', onRemove: () => removed++ })
  filter.querySelector('button').click()
  assert.equal(removed, 1)
  assert.equal(components.banner(doc, 'danger', 'x').getAttribute('role'), 'alert')
  assert.equal(components.banner(doc, 'info', 'x').getAttribute('role'), 'status')

  const search = components.searchBox(doc, { placeholder: '검색', label: '검색', scopes: [{ value: 'all', label: 'all' }, { value: 'id', label: 'id' }], hint: '/', invalid: true })
  assert.equal(search.input.getAttribute('aria-invalid'), 'true')
  assert.equal(search.select.options.length, 2)
  assert.equal(search.el.querySelector('kbd').textContent, '/')

  const picked = []
  const menu = components.menu(doc, [{ id: 'a', label: 'A', checked: true }, 'separator', { id: 'b', label: 'B', disabled: true }, { id: 'c', label: 'C' }], (id) => picked.push(id))
  menu.querySelectorAll('.m-menu-item').forEach((item) => item.click())
  assert.deepEqual(picked, ['a', 'c'])
  assert.equal(menu.querySelector('[data-id=a]').getAttribute('role'), 'menuitemcheckbox')
  assert.equal(menu.querySelector('[data-id=a]').getAttribute('aria-checked'), 'true')
})

test('tabs: roving tabindex, arrows wrap, Home/End, onSelect fires once per change', () => {
  const chosen = []
  const tabs = tabsModule.createTabs(doc, { selected: 'board', onSelect: (id) => chosen.push(id) })
  doc.body.append(tabs.el)
  const buttons = [...tabs.el.querySelectorAll('[role=tab]')]
  assert.deepEqual(buttons.map((b) => b.textContent), ['칸반', '목록', '위키'])
  const state = () => buttons.map((b) => `${b.getAttribute('aria-selected')}/${b.tabIndex}`)
  assert.deepEqual(state(), ['true/0', 'false/-1', 'false/-1'])
  assert.equal(tabs.el.getAttribute('role'), 'tablist')

  press(buttons[0], 'ArrowRight')
  assert.deepEqual(state(), ['false/-1', 'true/0', 'false/-1'])
  assert.equal(doc.activeElement, buttons[1])
  press(buttons[1], 'ArrowRight')
  press(buttons[2], 'ArrowRight')
  assert.equal(tabs.selected(), 'board', 'wraps to the first')
  press(buttons[0], 'ArrowLeft')
  assert.equal(tabs.selected(), 'wiki', 'wraps to the last')
  press(buttons[2], 'Home')
  assert.equal(tabs.selected(), 'board')
  press(buttons[0], 'End')
  assert.equal(tabs.selected(), 'wiki')
  assert.equal(press(buttons[2], 'a').defaultPrevented, false)
  buttons[2].click()
  assert.deepEqual(chosen, ['list', 'wiki', 'board', 'wiki', 'board', 'wiki'])

  tabs.setSelected('list')
  assert.deepEqual(state(), ['false/-1', 'true/0', 'false/-1'])
  assert.equal(chosen.length, 6, 'setSelected is silent')
  assert.equal(tabs.el.hasAttribute('data-badged'), false)
  tabs.setBadge('list', 3)
  // The label is hidden on a narrow bar and the badge would be the whole name: the tab is named with its count.
  assert.equal(buttons[1].getAttribute('aria-label'), '목록 3')
  // The strip is wider with a count: the style sheet gives the labels way sooner while one shows (components.css).
  assert.equal(tabs.el.hasAttribute('data-badged'), true)
  tabs.setBadge('list', 120)
  assert.equal(buttons[1].querySelector('.m-chip').textContent, '99+')
  assert.equal(buttons[1].getAttribute('aria-label'), '목록 99+')
  assert.equal(buttons[0].getAttribute('aria-label'), null, 'a tab with no badge keeps `title` for its name')
  tabs.setBadge('list', 0)
  assert.equal(buttons[1].querySelector('.m-chip'), null)
  assert.equal(buttons[1].getAttribute('aria-label'), null)
  assert.equal(tabs.el.hasAttribute('data-badged'), false)
  tabs.el.remove()
})

test('tabs: unknown stored tab falls back; panels keep their DOM and scroll', () => {
  assert.equal(tabsModule.normalizeTab('wiki'), 'wiki')
  assert.equal(tabsModule.normalizeTab('nope'), tabsModule.DEFAULT_TAB)
  assert.equal(tabsModule.normalizeTab(undefined), tabsModule.DEFAULT_TAB)

  const panels = tabsModule.createTabPanels(doc, 'list')
  const visible = () => [...panels.el.children].filter((p) => !p.hidden).map((p) => p.id)
  assert.deepEqual(visible(), ['m-panel-list'])
  const marker = doc.createElement('span')
  const scroller = doc.createElement('div')
  scroller.dataset.keepScroll = ''
  scroller.append(marker)
  panels.panel('list').append(scroller)
  scroller.scrollTop = 120

  panels.show('wiki')
  assert.deepEqual(visible(), ['m-panel-wiki'])
  assert.ok(panels.panel('list').contains(marker), 'the hidden panel keeps what it built')
  scroller.scrollTop = 0 // what display:none does to it
  panels.show('list')
  assert.equal(scroller.scrollTop, 120)
  assert.equal(panels.panel('list').getAttribute('aria-labelledby'), 'm-tab-list')

  // A view that re-renders while hidden: offsets go back to the elements they came from, not by position.
  panels.show('wiki')
  scroller.scrollTop = 0
  const added = doc.createElement('div')
  added.dataset.keepScroll = ''
  panels.panel('list').prepend(added)
  panels.show('list')
  assert.equal(scroller.scrollTop, 120)
  assert.equal(added.scrollTop, 0)
})

test('responsive: width below 640 forces bottom; columns drop date, epic, assignee, tags', () => {
  assert.equal(responsive.effectivePosition(639, 'left'), 'bottom')
  assert.equal(responsive.effectivePosition(640, 'left'), 'left')
  assert.equal(responsive.effectivePosition(280, 'bottom'), 'bottom')
  assert.equal(responsive.canChoosePosition(639), false)
  assert.equal(responsive.canChoosePosition(640), true)
  const all = ['created', 'updated', 'epic', 'assignee', 'tags']
  assert.deepEqual(responsive.visibleColumns(600, all), all)
  assert.deepEqual(responsive.visibleColumns(500, all), ['epic', 'assignee', 'tags'])
  assert.deepEqual(responsive.visibleColumns(450, all), ['assignee', 'tags'])
  assert.deepEqual(responsive.visibleColumns(399, all), ['tags'])
  assert.deepEqual(responsive.visibleColumns(280, all), [])
  assert.deepEqual(responsive.visibleColumns(450, ['tags', 'epic']), ['tags'], 'only enabled columns are kept')
  assert.equal(responsive.toolbarCompact(499), true)
  assert.equal(responsive.toolbarCompact(500), false)
})

test('responsive: the css container queries carry the same numbers as responsive.ts', () => {
  for (const { column, below } of responsive.COLUMN_DROPS) {
    const cls = column === 'created' || column === 'updated' ? 'date' : column
    assert.match(componentsCss, new RegExp(`@container m-list \\(max-width: ${below - 1}px\\) \\{[^}]*\\.m-col-${cls}\\b`), `${column} < ${below}`)
  }
  assert.match(componentsCss, new RegExp(`@container m-toolbar \\(max-width: ${responsive.TOOLBAR_COMPACT_BELOW - 1}px\\)`))
})

test('split: ratios are clamped to 15-85% and keep minimum sizes', () => {
  const { clampRatio, normalizeRatio, ratioFromPointer, DEFAULT_RATIO } = splitModule
  assert.deepEqual(DEFAULT_RATIO, { left: 0.45, bottom: 0.3 })
  assert.equal(normalizeRatio('x', 'left'), 0.45)
  assert.equal(normalizeRatio(NaN, 'bottom'), 0.3)
  assert.equal(normalizeRatio(0.01, 'left'), 0.15)
  assert.equal(normalizeRatio(2, 'left'), 0.85)
  assert.equal(clampRatio(0.5, 0, 'left'), 0.5, 'unknown size: only the percentage bounds')
  assert.equal(clampRatio(0.15, 1000, 'left'), 0.24, 'detail keeps 240px')
  assert.equal(clampRatio(0.85, 1000, 'left'), 0.84, 'list keeps 160px')
  assert.equal(clampRatio(0.15, 400, 'bottom'), 0.3, 'detail keeps its 120px')
  assert.equal(clampRatio(0.85, 400, 'bottom'), 0.79, 'list keeps 3 rows')
  assert.equal(clampRatio(0.5, 300, 'left'), 0.5, 'too small for both minimums: percentage bounds win')
  // pointer: left detail share grows with x, bottom detail share grows as y falls
  assert.equal(ratioFromPointer(500, 1000, 'left'), 0.5)
  assert.equal(Number(ratioFromPointer(700, 1000, 'bottom').toFixed(2)), 0.3)
})

test('split: position, forced bottom, ratios per position, visibility', () => {
  const calls = []
  const split = splitModule.createSplit(doc, { position: 'left', ratios: { left: 0.4 }, onRatio: (p, r) => calls.push([p, r]) })
  assert.equal(split.el.dataset.position, 'left')
  assert.equal(split.el.style.getPropertyValue('--m-detail-size'), '40.00%')
  assert.equal(split.ratio('bottom'), 0.3)
  assert.equal(split.el.children[1], split.resizer)
  assert.equal(split.resizer.getAttribute('role'), 'separator')
  assert.equal(split.resizer.getAttribute('aria-orientation'), 'vertical')

  split.setWidth(500)
  assert.equal(split.position(), 'bottom', 'forced')
  assert.equal(split.preferred(), 'left', 'the preference survives')
  assert.equal(split.el.dataset.position, 'bottom')
  assert.equal(split.resizer.getAttribute('aria-orientation'), 'horizontal')
  assert.equal(split.el.style.getPropertyValue('--m-detail-size'), '30.00%', 'bottom has its own ratio')
  split.setWidth(900)
  assert.equal(split.position(), 'left')
  split.setPosition('bottom')
  assert.equal(split.position(), 'bottom')

  split.setDetailVisible(false)
  assert.equal(split.el.dataset.detail, 'hidden')
  split.setDetailVisible(true)
  assert.equal(split.el.dataset.detail, 'shown')
  assert.deepEqual(calls, [])
})

test('split: arrow keys resize the detail and report each change', () => {
  const calls = []
  const split = splitModule.createSplit(doc, { position: 'left', ratios: { left: 0.45 }, onRatio: (p, r) => calls.push([p, Number(r.toFixed(2))]) })
  press(split.resizer, 'ArrowRight')
  press(split.resizer, 'ArrowLeft')
  press(split.resizer, 'ArrowLeft')
  press(split.resizer, 'Home')
  press(split.resizer, 'End')
  assert.equal(press(split.resizer, 'x').defaultPrevented, false)
  assert.deepEqual(calls, [['left', 0.47], ['left', 0.45], ['left', 0.43], ['left', 0.15], ['left', 0.85]])
  assert.equal(split.resizer.getAttribute('aria-valuenow'), '85')

  split.setPosition('bottom')
  press(split.resizer, 'ArrowUp') // divider up: the bottom detail grows
  press(split.resizer, 'ArrowDown', { shiftKey: true })
  assert.deepEqual(calls.slice(5), [['bottom', 0.32], ['bottom', 0.22]])
  assert.equal(split.ratio('left'), 0.85, 'left kept its own')
})

test('split: dragging the resizer sets the ratio and reports once at the end', () => {
  const calls = []
  const split = splitModule.createSplit(doc, { position: 'bottom', onRatio: (p, r) => calls.push([p, Number(r.toFixed(2))]) })
  doc.body.append(split.el)
  split.el.getBoundingClientRect = () => ({ left: 0, top: 0, width: 800, height: 1000, right: 800, bottom: 1000 })
  const fire = (type, init) => split.resizer.dispatchEvent(Object.assign(new dom.window.Event(type, { bubbles: true, cancelable: true }), init))
  fire('pointerdown', { pointerId: 1, button: 2, clientX: 0, clientY: 700 })
  assert.ok(!doc.body.classList.contains('m-resizing'), 'a right click does not start a drag')
  fire('pointerdown', { pointerId: 1, button: 0, clientX: 0, clientY: 700 })
  assert.ok(doc.body.classList.contains('m-resizing'))
  fire('pointermove', { clientX: 0, clientY: 500 })
  fire('pointermove', { clientX: 0, clientY: 400 })
  assert.deepEqual(calls, [], 'nothing reported while dragging')
  assert.equal(split.ratio('bottom'), 0.6)
  fire('pointerup', {})
  assert.deepEqual(calls, [['bottom', 0.6]])
  assert.ok(!doc.body.classList.contains('m-resizing'))
  split.destroy()
  split.el.remove()
})

test('position toggle: one icon button flips left/bottom and is disabled with a reason when narrow', () => {
  const labels = { toLeft: '상세를 왼쪽으로', toBottom: '상세를 아래로', forcedBottom: '폭이 좁아 아래로 고정' }
  const toggled = []
  const toggle = splitModule.createPositionToggle(doc, { position: 'bottom', width: 900, labels, onToggle: (next) => toggled.push(next) })
  const button = toggle.el
  assert.ok(button.classList.contains('m-iconbtn'))
  assert.equal(button.getAttribute('aria-label'), '상세를 왼쪽으로')
  assert.equal(button.title, '상세를 왼쪽으로')
  button.click()
  assert.deepEqual(toggled, ['left'])

  toggle.update('left', 900)
  assert.equal(button.getAttribute('aria-label'), '상세를 아래로')
  assert.equal(button.querySelectorAll('svg').length, 1)
  button.click()
  assert.deepEqual(toggled, ['left', 'bottom'])

  toggle.update('bottom', 500)
  assert.equal(button.getAttribute('aria-disabled'), 'true')
  assert.equal(button.title, '폭이 좁아 아래로 고정')
  button.click()
  assert.equal(toggled.length, 2, 'does nothing while forced')
  toggle.update('bottom', 640)
  assert.equal(button.hasAttribute('aria-disabled'), false)
})
