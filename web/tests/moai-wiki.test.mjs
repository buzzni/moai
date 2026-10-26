// Moai panel wiki tab: page list and body (se-rhu3.z9l), search and backlinks (se-rhu3.28f), following links
// (se-rhu3.oej). Fixtures in tests/fixtures/moai-wiki/ are real `moai wiki ls/show --json` output.
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { test } from 'node:test'
import { JSDOM } from 'jsdom'

const panel = new URL('../panel/', import.meta.url)
const fixtures = new URL('./fixtures/moai-wiki/', import.meta.url)
const wiki = await import(new URL('view/wiki.ts', panel))
const wikiSource = await readFile(new URL('view/wiki.ts', panel), 'utf8')
const wikiCss = await readFile(new URL('style/wiki.css', panel), 'utf8')
const { COPY } = await import(new URL('i18n/index.ts', panel))
const fixture = async (name) => JSON.parse(await readFile(new URL(name, fixtures), 'utf8'))
const ls = await fixture('wiki-ls.json')
const shows = {
  'moai-panel/keybindings': await fixture('wiki-show-keybindings.json'),
  'moai-panel/overview': await fixture('wiki-show-overview.json'),
  USER_GUIDE: await fixture('wiki-show-user-guide.json'),
}

const dom = new JSDOM('<!doctype html><body></body>', { pretendToBeVisual: true })
const doc = dom.window.document
const press = (node, key, init = {}) => {
  const event = new dom.window.KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...init })
  node.dispatchEvent(event)
  return event
}
const tick = () => new Promise((resolve) => setTimeout(resolve, 0))
const deferred = () => {
  let resolve, reject
  const promise = new Promise((res, rej) => { resolve = res; reject = rej })
  return { promise, resolve, reject }
}

// A small extra page with every link kind, so the follow tests do not depend on what the real docs say.
const PLAYGROUND = {
  slug: 'moai-panel/playground',
  title: 'Playground',
  path: 'docs/moai-panel/playground.md',
  bytes: 300,
  issues: [{ id: 'se-rhu3.oej', exists: true }],
  links: [],
  linked_from: ['moai-panel/overview'],
  conflict: false,
  body: [
    '# Playground',
    '',
    'Go [keys](keybindings.md#3.8%20위키%20탭), [top](#Second%20Part), [out](https://example.com/x), work on se-rhu3.oej.',
    '',
    '## Second Part',
    '',
    'tail <script>alert(1)</script>',
  ].join('\n'),
}
const allPages = () => [...ls.pages, PLAYGROUND]

function makeView(extra = {}) {
  const calls = { load: [], issue: [], ratio: [] }
  const view = wiki.createWikiView(doc, {
    loadPage: (slug) => {
      calls.load.push(slug)
      if (slug === PLAYGROUND.slug) return Promise.resolve(structuredClone(PLAYGROUND))
      const show = shows[slug]
      if (show) return Promise.resolve(structuredClone(show))
      const listed = ls.pages.find((p) => p.slug === slug)
      return listed ? Promise.resolve({ ...structuredClone(listed), body: `# ${listed.title}\n\nbody of ${slug}\n` }) : Promise.reject(new Error(`no page ${slug}`))
    },
    onIssue: (id) => calls.issue.push(id),
    onRatio: (r) => calls.ratio.push(r),
    flashMs: 20,
    ...extra,
  })
  doc.body.replaceChildren(view.el)
  return { view, calls }
}
const scroller = (view) => view.el.querySelector('.m-wiki-scroll')
const rows = (view) => [...view.el.querySelectorAll('.m-wiki-list .m-row')]
const rowSlugs = (view) => rows(view).map((r) => r.dataset.slug)

// ---- pure logic ----

test('parseWikiLs / parseWikiShow: real output, text or parsed, and readable errors', () => {
  const parsed = wiki.parseWikiLs(JSON.stringify(ls))
  assert.equal(parsed.ok, true)
  assert.equal(parsed.value.dir, 'docs')
  assert.equal(parsed.value.pages.length, 8)
  assert.deepEqual(parsed.value.pages[1].linked_from, ['moai-panel/overview', 'moai-panel/design', 'moai-panel/keybindings'])
  const show = wiki.parseWikiShow(shows['moai-panel/keybindings'])
  assert.equal(show.ok, true)
  assert.match(show.value.body, /^# Moai 패널 단축키 매핑/)
  // Only the slug is required; the rest falls back.
  const bare = wiki.parseWikiShow({ slug: 'x' })
  assert.deepEqual(bare.value, { slug: 'x', title: 'x', path: '', issues: [], links: [], linked_from: [], conflict: false })
  // The folder cannot be used: `{dir, error}`.
  const refused = wiki.parseWikiLs({ dir: 'docs', error: { kind: 'missing', said: 'docs 폴더가 없습니다' } })
  assert.deepEqual(refused, { ok: false, error: 'docs 폴더가 없습니다' })
  assert.equal(wiki.parseWikiLs('{oops').ok, false)
  assert.match(wiki.parseWikiLs('{oops').error, /읽지 못했습니다/)
  assert.equal(wiki.parseWikiLs({ dir: 'docs' }).ok, false)
  assert.equal(wiki.parseWikiShow([1]).ok, false)
  // A page whose body could not be read keeps its error.
  const unread = wiki.parseWikiShow({ slug: 'big', error: { kind: 'too_large', said: '1MB가 넘습니다' }, bytes: 2e6 })
  assert.deepEqual(unread.value.error, { kind: 'too_large', said: '1MB가 넘습니다' })
})

test('sortPages: README and index first (root before folders), then title order, stable', () => {
  // The real listing is already in moai's order and sortPages keeps it.
  const real = wiki.sortPages([...ls.pages].reverse()).map((p) => p.slug)
  assert.deepEqual(real, ls.pages.map((p) => p.slug))
  const pages = [
    { slug: 'Zeta', title: 'Zeta' },
    { slug: 'guide/README', title: 'Guide home' },
    { slug: 'index', title: 'Index page' },
    { slug: 'alpha', title: 'alpha' },
    { slug: 'Readme', title: 'Home' },
    { slug: 'guide/Index', title: 'Guide index' },
    { slug: '안내', title: '안내' },
    { slug: 'beta', title: 'Same' },
    { slug: 'a-beta', title: 'Same' },
  ]
  assert.deepEqual(wiki.sortPages(pages).map((p) => p.slug), ['Readme', 'index', 'guide/README', 'guide/Index', 'alpha', 'a-beta', 'beta', 'Zeta', '안내'])
  assert.equal(pages[0].slug, 'Zeta', 'the input is not reordered')
  // Code-point order, as Rust compares: U+FF21 before an astral emoji (UTF-16 `<` would flip them).
  assert.deepEqual(wiki.sortPages([{ slug: 'e', title: '\u{1F4D8} book' }, { slug: 'f', title: 'Ａ wide' }]).map((p) => p.slug), ['f', 'e'])
})

test('pageWarnings: conflict, unreadable and over 1 MB', () => {
  assert.deepEqual(wiki.pageWarnings({ conflict: false, bytes: 10 }), [])
  assert.deepEqual(wiki.pageWarnings({ conflict: true }).map((w) => [w.kind, w.label]), [['conflict', '충돌']])
  const failed = wiki.pageWarnings({ conflict: false, error: { kind: 'failed', said: 'not UTF-8' } })
  assert.deepEqual(failed.map((w) => [w.label, w.detail]), [['못 읽음', 'not UTF-8']])
  assert.deepEqual(wiki.pageWarnings({ conflict: false, bytes: 1024 * 1024 + 1 }).map((w) => w.label), ['1MB 초과'])
  // too_large is the size warning, not a second "unreadable" chip.
  assert.deepEqual(wiki.pageWarnings({ conflict: true, bytes: 3e6, error: { kind: 'too_large', said: 'big' } }).map((w) => w.label), ['충돌', '1MB 초과'])
})

test('resolveWikiTarget: the port of moai wiki::target', () => {
  const r = wiki.resolveWikiTarget
  assert.deepEqual(r('moai-panel/overview', 'design.md'), { kind: 'wiki', slug: 'moai-panel/design' })
  assert.deepEqual(r('moai-panel/overview', './design.md'), { kind: 'wiki', slug: 'moai-panel/design' })
  assert.deepEqual(r('moai-panel/overview', '../USER_GUIDE.md'), { kind: 'wiki', slug: 'USER_GUIDE' })
  assert.deepEqual(r('guide/a', 'sub/../b.md'), { kind: 'wiki', slug: 'guide/b' })
  assert.deepEqual(r('README', 'guide/Beta.md'), { kind: 'wiki', slug: 'guide/Beta' })
  assert.deepEqual(r('README', 'my%20page.md'), { kind: 'wiki', slug: 'my page' })
  assert.deepEqual(r('README', '%EC%95%88%EB%82%B4.md'), { kind: 'wiki', slug: '안내' })
  assert.deepEqual(r('README', 'bad%ZZ.md'), { kind: 'wiki', slug: 'bad%ZZ' }, 'malformed %XX stays as written')
  assert.deepEqual(r('README', '%EF%BB%BFa.md'), { kind: 'wiki', slug: '﻿a' }, 'a decoded BOM is kept, as String::from_utf8 keeps it')
  assert.deepEqual(r('README', 'a.md?plain=1'), { kind: 'wiki', slug: 'a' })
  // #anchor: decoded and GitHub-slugged.
  assert.deepEqual(r('moai-panel/overview', 'keybindings.md#3.8 위키 탭'), { kind: 'wiki', slug: 'moai-panel/keybindings', anchor: '38-위키-탭' })
  assert.deepEqual(r('README', 'glossary.md#Start%20Here'), { kind: 'wiki', slug: 'glossary', anchor: 'start-here' })
  assert.deepEqual(r('README', 'glossary.md#'), { kind: 'wiki', slug: 'glossary' }, 'an empty fragment is no anchor')
  assert.deepEqual(r('README', '#Start Here'), { kind: 'anchor', anchor: 'start-here' })
  assert.deepEqual(r('README', '#x'), { kind: 'anchor', anchor: 'x' })
  assert.deepEqual(r('README', '#'), { kind: 'external', href: '#' })
  // Outside the wiki.
  for (const href of ['/abs.md', '\\abs.md', 'https://example.com/a.md', 'mailto:a@b.c', 'javascript:alert(1)//.md', 'notes.txt', 'dir/', '../x.md', 'a/../../x.md', '..md', '.md', 'sub/..']) {
    assert.deepEqual(r('README', href), { kind: 'external', href }, href)
  }
  assert.deepEqual(r('moai-panel/overview', '../../x.md'), { kind: 'external', href: '../../x.md' }, 'escaping the root')
  // With the known pages, a link to a page that is not there is shown, never followed.
  const known = ls.pages.map((p) => p.slug)
  assert.deepEqual(r('moai-panel/overview', 'design.md', known), { kind: 'wiki', slug: 'moai-panel/design' })
  assert.deepEqual(r('moai-panel/overview', 'gone.md', known), { kind: 'external', href: 'gone.md' })
})

test('filterPages: title and known bodies, case-insensitive', () => {
  const pages = ls.pages
  assert.equal(wiki.filterPages(pages, '').length, 8)
  assert.deepEqual(wiki.filterPages(pages, 'MOAI 패널').map((p) => p.slug), ['moai-panel/overview', 'moai-panel/design', 'moai-panel/keybindings', 'moai-panel/look-and-feel'])
  // A word only in a body matches only once that body is known.
  assert.deepEqual(wiki.filterPages(pages, 'before-input-event'), [])
  const bodies = new Map([['moai-panel/keybindings', shows['moai-panel/keybindings'].body]])
  assert.deepEqual(wiki.filterPages(pages, 'BEFORE-INPUT-EVENT', bodies).map((p) => p.slug), ['moai-panel/keybindings'])
  assert.deepEqual(wiki.filterPages(pages, 'before-input-event', { 'moai-panel/keybindings': 'x before-input-event' }).map((p) => p.slug), ['moai-panel/keybindings'])
})

// moai-xp7l.nyi: the list for a query is moai's own rank first, then what the substring finds that it did not rank.
test('rankPages and parseWikiSearch: moai\'s rank, then the substring', () => {
  const pages = [
    { slug: 'README', title: 'Home' },
    { slug: 'a', title: 'A zebra page' },
    { slug: 'b', title: 'Zebras' },
    { slug: 'c', title: 'Other', body: 'zebra once' },
  ]
  const hits = { dir: 'docs', query: 'zebra', terms: ['zebra'], hits: [{ slug: 'c' }, { slug: 'a' }, { slug: 'c' }, { slug: 'gone' }], total: 4 }
  assert.deepEqual(wiki.parseWikiSearch(hits), ['c', 'a', 'gone'], 'each slug once, best first')
  assert.deepEqual(wiki.parseWikiSearch(JSON.stringify(hits)), ['c', 'a', 'gone'], 'printed text reads the same')
  assert.equal(wiki.parseWikiSearch({ dir: 'docs', error: { kind: 'no_dir' } }), null)
  const ranked = wiki.parseWikiSearch(hits)
  assert.deepEqual(wiki.rankPages(pages, 'zebra', undefined, ranked).map((p) => p.slug), ['c', 'a', 'b'], 'a slug the list lacks is passed over')
  assert.deepEqual(wiki.rankPages(pages, 'zebra', undefined, null).map((p) => p.slug), ['a', 'b', 'c'])
  assert.deepEqual(wiki.rankPages(pages, '', undefined, ranked).map((p) => p.slug), ['README', 'a', 'b', 'c'], 'no query, no rank')
})

test('the search field asks moai for the rank once the keys stop and drops a stale answer', async () => {
  const asked = []
  const answers = {}
  const { view } = makeView({
    searchPages: (q) => {
      asked.push(q)
      return answers[q] ?? Promise.reject(new Error('no words'))
    },
  })
  view.setPages(ls.pages, 'docs')
  await tick()
  const before = rowSlugs(view)
  answers['moai'] = Promise.resolve({ hits: [...before].reverse().map((slug) => ({ slug })) })
  view.setQuery('mo')
  view.setQuery('moai')
  await new Promise((resolve) => setTimeout(resolve, wiki.RANK_DELAY_MS + 20))
  assert.deepEqual(asked, ['moai'], 'asked once, for the last query')
  const ranked = rowSlugs(view)
  assert.equal(ranked[0], [...before].reverse().find((slug) => ranked.includes(slug)), 'the rank leads the list')
  // A failed rank (a query with no words) keeps the substring list.
  view.setQuery('!!!')
  await new Promise((resolve) => setTimeout(resolve, wiki.RANK_DELAY_MS + 20))
  assert.deepEqual(rowSlugs(view), wiki.filterPages(ls.pages, '!!!').map((p) => p.slug))
  view.destroy()
})

test('trail and link cycling helpers', () => {
  let trail = wiki.pushTrail([], { slug: 'a', scrollTop: 10 })
  trail = wiki.pushTrail(trail, { slug: 'b', scrollTop: -5 })
  assert.deepEqual(trail, [{ slug: 'a', scrollTop: 10 }, { slug: 'b', scrollTop: 0 }])
  assert.deepEqual(wiki.popTrail(trail), { entry: { slug: 'b', scrollTop: 0 }, trail: [{ slug: 'a', scrollTop: 10 }] })
  assert.equal(wiki.popTrail([]), null)
  assert.equal(wiki.pushTrail(trail, { slug: 'c', scrollTop: 0 }, 2).length, 2)
  assert.equal(wiki.cycleIndex(-1, 1, 3), 0)
  assert.equal(wiki.cycleIndex(-1, -1, 3), 2)
  assert.equal(wiki.cycleIndex(2, 1, 3), 0)
  assert.equal(wiki.cycleIndex(0, -1, 3), 2)
  assert.equal(wiki.cycleIndex(0, 1, 0), -1)
})

// ---- the view ----

test('view: list on the left with warning chips, first page opened, header shows title and path', async () => {
  const { view, calls } = makeView()
  const pages = structuredClone(ls.pages)
  pages[3].conflict = true
  pages[6].bytes = 2 * 1024 * 1024
  view.setPages(pages.reverse(), 'docs')
  assert.deepEqual(rowSlugs(view), ls.pages.map((p) => p.slug))
  const design = rows(view).find((r) => r.dataset.slug === 'moai-panel/design')
  assert.deepEqual([...design.querySelectorAll('.m-chip')].map((c) => c.textContent), ['충돌'])
  assert.deepEqual([...rows(view).find((r) => r.dataset.slug === 'DEVELOPMENT').querySelectorAll('.m-chip')].map((c) => c.textContent), ['1MB 초과'])
  assert.equal(design.querySelector('.m-wiki-folder').textContent, 'moai-panel/')
  await tick()
  assert.deepEqual(calls.load, ['USER_GUIDE'])
  assert.equal(view.el.querySelector('.m-wiki-title').textContent, 'Installing and managing Saycode Extensions')
  assert.equal(view.el.querySelector('.m-wiki-path').textContent, 'docs/USER_GUIDE.md')
  assert.ok(scroller(view).querySelector('.m-md h2'))
  assert.equal(rows(view)[0].getAttribute('aria-current'), 'page')
  // The split is always left/right; the list holds the first pane and wiki_width is its ratio.
  const split = view.el.querySelector('.m-split')
  assert.equal(split.dataset.position, 'left')
  assert.ok(view.el.querySelector('.m-split-detail .m-wiki-list'))
  assert.ok(view.el.querySelector('.m-split-list .m-wiki-body'))
  view.setWidth(500)
  assert.equal(split.dataset.position, 'bottom', 'narrow: stacked')
  assert.equal(view.el.dataset.narrow, 'true')
  view.setWidth(900)
  assert.equal(split.dataset.position, 'left')
  press(view.el.querySelector('.m-split-resizer'), 'ArrowRight')
  assert.equal(calls.ratio.length, 1)
})

test('view: list keys move the cursor and Enter opens; list error, loading and empty states', async () => {
  const { view, calls } = makeView()
  view.setLoading(true)
  assert.match(view.el.querySelector('.m-wiki-list-note').textContent, /불러오는 중/)
  view.setError('docs 폴더가 없습니다')
  assert.equal(view.el.querySelector('.m-wiki-list-note [role="alert"]').textContent, 'docs 폴더가 없습니다')
  view.setPages(ls.pages)
  assert.equal(view.el.querySelector('.m-wiki-list-note').children.length, 0)
  await tick()
  view.focusPane('list')
  press(doc.activeElement, 'j')
  press(doc.activeElement, 'j')
  assert.equal(view.state().cursor, 'moai-panel/overview')
  press(doc.activeElement, 'Enter')
  await tick()
  assert.deepEqual(calls.load, ['USER_GUIDE', 'moai-panel/overview'])
  assert.equal(view.state().current, 'moai-panel/overview')
  assert.deepEqual(view.state().trail, [], 'opening from the list is not a link follow')
  rows(view).find((r) => r.dataset.slug === 'DEVELOPMENT').click()
  await tick()
  assert.equal(view.state().current, 'DEVELOPMENT')
  const empty = makeView().view
  empty.setPages([], 'docs')
  assert.match(empty.el.querySelector('.m-wiki-list-note').textContent, /위키 페이지가 없습니다/)
})

test('view: search narrows the list by title and loaded bodies; / focuses it; Esc clears', async () => {
  const { view } = makeView()
  view.setPages(ls.pages)
  await tick()
  press(scroller(view), '/')
  const input = view.el.querySelector('.m-wiki-search input')
  assert.equal(doc.activeElement, input)
  input.value = '세이코드'
  input.dispatchEvent(new dom.window.Event('input', { bubbles: true }))
  assert.deepEqual(rowSlugs(view), ['moai-panel/look-and-feel'])
  // A word only in a body: found once that page has been loaded.
  view.setQuery('permission meanings')
  assert.deepEqual(rowSlugs(view), ['USER_GUIDE'], 'USER_GUIDE was loaded on start')
  view.setQuery('before-input-event')
  assert.deepEqual(rowSlugs(view), [])
  assert.match(view.el.querySelector('.m-wiki-list-note').textContent, /찾는 페이지가 없습니다/)
  view.setBody('moai-panel/keybindings', shows['moai-panel/keybindings'].body)
  assert.deepEqual(rowSlugs(view), ['moai-panel/keybindings'])
  // Esc in the field clears and goes back to the list.
  press(input, 'Escape')
  assert.equal(view.state().query, '')
  assert.equal(input.value, '')
  assert.equal(rows(view).length, 8)
  assert.equal(view.state().pane, 'list')
})

test('view: backlinks at the end are wiki links that load their page', async () => {
  const { view, calls } = makeView()
  view.setPages(ls.pages)
  await view.open('moai-panel/keybindings')
  const section = scroller(view).querySelector('.m-wiki-backlinks')
  assert.equal(section, scroller(view).lastElementChild)
  const backlinks = [...section.querySelectorAll('a.m-md-link')]
  assert.deepEqual(backlinks.map((a) => [a.dataset.kind, a.dataset.slug, a.textContent]), [
    ['wiki', 'moai-panel/tui-analysis', 'moai TUI 분석'],
    ['wiki', 'moai-panel/overview', 'Moai 패널 2.0 개편 계획'],
    ['wiki', 'moai-panel/design', 'Moai 패널 2.0 설계'],
  ])
  assert.ok(backlinks.every((a) => !a.hasAttribute('href')))
  // Backlinks are in the Tab order after the body links.
  assert.deepEqual(view.links().slice(-3), backlinks)
  backlinks[1].click()
  await tick()
  assert.equal(view.state().current, 'moai-panel/overview')
  assert.deepEqual(view.state().trail.map((e) => e.slug), ['moai-panel/keybindings'])
  assert.equal(calls.load.at(-1), 'moai-panel/overview')
  // No backlinks: says so.
  await view.open('USER_GUIDE')
  assert.match(scroller(view).querySelector('.m-wiki-backlinks').textContent, /가리키는 문서가 없습니다/)
})

test('view: Tab / Shift-Tab cycle body links with wrap; Enter on a wiki link loads it and pushes the trail', async () => {
  const { view, calls } = makeView()
  view.setPages(ls.pages)
  await view.open('moai-panel/keybindings')
  view.focusPane('body')
  const links = view.links()
  assert.deepEqual(links.slice(0, 2).map((a) => [a.dataset.kind, a.dataset.slug]), [['wiki', 'moai-panel/tui-analysis'], ['wiki', 'moai-panel/overview']])
  // The real page writes its issue id in a code span; a span that is exactly an issue id is an issue link.
  const issueLinks = links.filter((a) => a.dataset.kind === 'issue')
  assert.ok(issueLinks.length > 0)
  assert.ok(issueLinks.every((a) => a.firstElementChild?.tagName === 'CODE'))
  const issueIndex = links.indexOf(issueLinks[0])
  for (let i = 0; i <= issueIndex; i++) press(scroller(view), 'Tab')
  press(doc.activeElement, 'Enter')
  await tick()
  assert.deepEqual(calls.issue, [issueLinks[0].dataset.issue])
  view.clearLinkSelection()
  const selectedIndex = () => links.findIndex((a) => a.classList.contains('m-wiki-link-selected'))
  press(scroller(view), 'Tab')
  assert.equal(selectedIndex(), 0)
  assert.equal(doc.activeElement, links[0])
  assert.equal(links[0].tabIndex, 0)
  assert.equal(links[0].getAttribute('aria-current'), 'true')
  press(doc.activeElement, 'Tab', { shiftKey: true })
  assert.equal(selectedIndex(), links.length - 1, 'Shift-Tab wraps to the last')
  press(doc.activeElement, 'Tab')
  assert.equal(selectedIndex(), 0, 'Tab wraps to the first')
  assert.equal(links.filter((a) => a.tabIndex === 0).length, 1, 'roving tabindex')
  scroller(view).scrollTop = 250
  const event = press(doc.activeElement, 'Enter')
  assert.equal(event.defaultPrevented, true)
  await tick()
  assert.equal(calls.load.at(-1), 'moai-panel/tui-analysis')
  assert.equal(view.state().current, 'moai-panel/tui-analysis')
  assert.deepEqual(view.state().trail, [{ slug: 'moai-panel/keybindings', scrollTop: 250 }])
  assert.equal(scroller(view).scrollTop, 0)
  assert.equal(view.state().selectedLink, -1, 'a new page starts with no link selected')
})

test('view: Backspace, h and ArrowLeft go back along the trail and restore the scroll', async () => {
  const { view } = makeView()
  view.setPages(allPages())
  await view.open('moai-panel/overview')
  view.focusPane('body')
  scroller(view).scrollTop = 120
  await view.follow({ kind: 'wiki', slug: 'moai-panel/keybindings' })
  scroller(view).scrollTop = 80
  await view.follow({ kind: 'wiki', slug: 'USER_GUIDE' })
  scroller(view).scrollTop = 40
  await view.follow({ kind: 'wiki', slug: 'moai-panel/design' })
  assert.deepEqual(view.state().trail.map((e) => e.slug), ['moai-panel/overview', 'moai-panel/keybindings', 'USER_GUIDE'])
  view.focusPane('body')
  press(scroller(view), 'Backspace')
  await tick()
  assert.equal(view.state().current, 'USER_GUIDE')
  assert.equal(scroller(view).scrollTop, 40)
  press(scroller(view), 'h')
  await tick()
  assert.equal(view.state().current, 'moai-panel/keybindings')
  assert.equal(scroller(view).scrollTop, 80)
  press(scroller(view), 'ArrowLeft')
  await tick()
  assert.equal(view.state().current, 'moai-panel/overview')
  assert.equal(scroller(view).scrollTop, 120)
  assert.equal(await view.back(), false, 'an empty trail stays put')
  assert.equal(view.state().current, 'moai-panel/overview')
  assert.match(view.el.querySelector('.m-wiki-status').textContent, /되감을 페이지가 없습니다/)
})

test('view: anchors scroll to the heading and flash it; same-page wiki anchors too', async () => {
  const { view, calls } = makeView()
  view.setPages(allPages())
  await view.open(PLAYGROUND.slug)
  const second = scroller(view).querySelector('#m-md-second-part')
  assert.ok(second)
  second.getBoundingClientRect = () => ({ top: 300, left: 0, right: 0, bottom: 320, width: 0, height: 20 })
  scroller(view).scrollTop = 10
  const top = view.links().find((a) => a.dataset.kind === 'anchor')
  assert.equal(top.dataset.anchor, 'second-part')
  top.click()
  assert.equal(scroller(view).scrollTop, 310)
  assert.ok(second.classList.contains('m-wiki-flash'))
  await new Promise((resolve) => setTimeout(resolve, 40))
  assert.ok(!second.classList.contains('m-wiki-flash'), 'the highlight is brief')
  assert.equal(view.state().trail.length, 0, 'an anchor is no trail step')
  assert.equal(view.scrollToAnchor('nowhere'), false)
  // A wiki link with an anchor into another page: loads it, then scrolls there.
  const keys = view.links().find((a) => a.dataset.kind === 'wiki' && a.dataset.slug === 'moai-panel/keybindings')
  assert.equal(keys.dataset.anchor, '38-위키-탭')
  keys.click()
  await tick()
  assert.equal(calls.load.at(-1), 'moai-panel/keybindings')
  const heading = scroller(view).querySelector('[data-anchor="38-위키-탭"]')
  assert.ok(heading?.classList.contains('m-wiki-flash'))
})

test('view: an issue link calls onIssue; an external link opens nothing', async () => {
  const opened = []
  const original = dom.window.open
  dom.window.open = (...args) => opened.push(args)
  try {
    const { view, calls } = makeView()
    view.setPages(allPages())
    await view.open(PLAYGROUND.slug)
    const before = calls.load.length
    const external = view.links().find((a) => a.dataset.kind === 'external')
    assert.equal(external.dataset.href, 'https://example.com/x')
    assert.equal(external.hasAttribute('href'), false)
    external.click()
    await tick()
    assert.deepEqual(opened, [])
    assert.equal(calls.load.length, before)
    assert.deepEqual(calls.issue, [])
    assert.equal(view.state().current, PLAYGROUND.slug)
    assert.match(view.el.querySelector('.m-wiki-status').textContent, /열지 않습니다: https:\/\/example.com\/x/)
    // Issue: via keyboard.
    assert.equal(view.state().selectedLink, view.links().indexOf(external), 'a click selects the link too')
    view.clearLinkSelection()
    view.focusPane('body')
    const links = view.links()
    const issueIndex = links.findIndex((a) => a.dataset.kind === 'issue')
    for (let i = 0; i <= issueIndex; i++) press(doc.activeElement, 'Tab')
    press(doc.activeElement, 'Enter')
    await tick()
    assert.deepEqual(calls.issue, ['se-rhu3.oej'])
    assert.equal(calls.load.length, before)
    // Raw HTML in the body is text, never markup.
    assert.equal(scroller(view).querySelector('script'), null)
    assert.match(scroller(view).textContent, /<script>alert\(1\)<\/script>/)
  } finally {
    dom.window.open = original
  }
})

test('view: Esc clears the link selection first, then the search', async () => {
  const { view } = makeView()
  view.setPages(ls.pages)
  await view.open('moai-panel/keybindings')
  view.setQuery('moai')
  view.focusPane('body')
  press(scroller(view), 'Tab')
  assert.equal(view.state().selectedLink, 0)
  let event = press(doc.activeElement, 'Escape')
  assert.equal(event.defaultPrevented, true)
  assert.equal(view.state().selectedLink, -1)
  assert.equal(view.state().query, 'moai')
  assert.equal(doc.activeElement, scroller(view))
  event = press(doc.activeElement, 'Escape')
  assert.equal(view.state().query, '')
  assert.equal(event.defaultPrevented, true)
  event = press(doc.activeElement, 'Escape')
  assert.equal(event.defaultPrevented, false, 'nothing left: Esc is not the wiki tab\'s')
})

test('view: focus stays in the view after Enter opens a row or follows a link, and after a refresh', async () => {
  const { view } = makeView()
  view.setPages(allPages())
  await tick()
  view.focusPane('list')
  press(doc.activeElement, 'j')
  press(doc.activeElement, 'Enter')
  await tick()
  assert.ok(view.el.contains(doc.activeElement), 'the repaint did not leave focus on <body>')
  const before = view.state().cursor
  press(doc.activeElement, 'j')
  assert.notEqual(view.state().cursor, before, 'j still reaches the view')
  // Esc on a focused row clears the search and repaints the list: focus stays on a row.
  view.setQuery('moai')
  view.focusPane('list')
  press(doc.activeElement, 'Escape')
  assert.equal(view.state().query, '')
  assert.equal(doc.activeElement.getAttribute('aria-selected'), 'true')
  // Follow a link with the keyboard, then go back with Backspace pressed where focus is.
  await view.open(PLAYGROUND.slug)
  view.focusPane('body')
  const wikiIndex = view.links().findIndex((a) => a.dataset.kind === 'wiki')
  for (let i = 0; i <= wikiIndex; i++) press(doc.activeElement, 'Tab')
  press(doc.activeElement, 'Enter')
  await tick()
  assert.equal(view.state().current, 'moai-panel/keybindings')
  assert.equal(doc.activeElement, scroller(view))
  press(doc.activeElement, 'Backspace')
  await tick()
  assert.equal(view.state().current, PLAYGROUND.slug)
  // A list refresh keeps the selected link and its focus.
  press(doc.activeElement, 'Tab')
  press(doc.activeElement, 'Tab')
  assert.equal(view.state().selectedLink, 1)
  view.setPages(allPages())
  assert.equal(view.state().selectedLink, 1)
  assert.equal(doc.activeElement, view.links()[1])
})

test('view: Enter on a focused button is the button\'s; h and ArrowLeft go back from the list too', async () => {
  const { view } = makeView()
  view.setPages(allPages())
  await view.open('moai-panel/overview')
  await view.follow({ kind: 'wiki', slug: PLAYGROUND.slug })
  const rawButton = [...view.el.querySelectorAll('.m-wiki-actions button')].find((b) => b.textContent === '원문')
  rawButton.focus()
  const event = press(rawButton, 'Enter')
  assert.equal(event.defaultPrevented, false)
  assert.equal(view.state().selectedLink, -1)
  view.focusPane('list')
  press(doc.activeElement, 'h')
  await tick()
  assert.equal(view.state().current, 'moai-panel/overview')
})

test('view: an issue id moai found no issue for stays text', async () => {
  const { view } = makeView({
    loadPage: () => Promise.resolve({ ...structuredClone(PLAYGROUND), issues: [{ id: 'se-rhu3.oej', exists: false }] }),
  })
  view.setPages(allPages())
  await view.open(PLAYGROUND.slug)
  assert.equal(view.links().filter((a) => a.dataset.kind === 'issue').length, 0)
  assert.match(scroller(view).textContent, /se-rhu3\.oej/)
})

test('view: Enter in the body with no link selected selects the first', async () => {
  const { view } = makeView()
  view.setPages(ls.pages)
  await view.open('moai-panel/keybindings')
  view.focusPane('body')
  press(scroller(view), 'Enter')
  assert.equal(view.state().selectedLink, 0)
})

test('view: toggleRaw shows the source as <pre> text and back', async () => {
  const { view } = makeView()
  view.setPages(allPages())
  await view.open(PLAYGROUND.slug)
  view.toggleRaw()
  const pre = scroller(view).querySelector('pre.m-wiki-raw')
  assert.equal(pre.textContent, PLAYGROUND.body)
  assert.equal(scroller(view).querySelector('.m-md'), null)
  assert.equal(view.el.querySelector('.m-wiki-actions [aria-pressed="true"]').textContent, '원문')
  assert.equal(view.state().raw, true)
  assert.ok(scroller(view).querySelector('.m-wiki-backlinks'), 'backlinks stay in raw view')
  view.toggleRaw()
  assert.ok(scroller(view).querySelector('.m-md'))
  assert.equal(scroller(view).querySelector('pre.m-wiki-raw'), null)
})

test('view: a failed load shows the error; a stale slower load never overwrites a later one', async () => {
  const pending = { a: deferred(), b: deferred() }
  const view = wiki.createWikiView(doc, { loadPage: (slug) => pending[slug]?.promise ?? Promise.reject(new Error('boom')) })
  view.setPages([{ slug: 'a', title: 'A', path: 'docs/a.md', issues: [], links: [], linked_from: [], conflict: false }, { slug: 'b', title: 'B', path: 'docs/b.md', issues: [], links: [], linked_from: [], conflict: false }])
  // setPages opened "a" (still pending); now open "b".
  assert.match(scroller(view).textContent, /불러오는 중/)
  const opening = view.open('b')
  pending.b.resolve({ slug: 'b', title: 'B', path: 'docs/b.md', body: '# B page\n\nthe later one' })
  await opening
  pending.a.resolve({ slug: 'a', title: 'A', path: 'docs/a.md', body: '# A page\n\nthe stale one' })
  await tick()
  assert.equal(view.state().current, 'b')
  assert.match(scroller(view).textContent, /the later one/)
  assert.doesNotMatch(scroller(view).textContent, /stale/)
  assert.equal(view.el.querySelector('.m-wiki-title').textContent, 'B')
  // A rejected load and an unparsable answer both show a readable error.
  await view.open('zzz')
  assert.match(scroller(view).querySelector('[role="alert"]').textContent, /페이지를 불러오지 못했습니다: boom/)
  pending.c = deferred()
  pending.c.resolve([1, 2])
  await view.open('c')
  assert.match(scroller(view).querySelector('[role="alert"]').textContent, /꼴이 맞지 않습니다/)
  // An unreadable page body: the reason, no render.
  pending.d = deferred()
  pending.d.resolve({ slug: 'd', title: 'D', error: { kind: 'too_large', said: '1MB가 넘어 읽지 않았습니다' } })
  await view.open('d')
  assert.match(scroller(view).textContent, /1MB가 넘어 읽지 않았습니다/)
})

test('view: state() and restore() carry the page, trail, search, raw and scroll across tab switches', async () => {
  const first = makeView()
  first.view.setPages(allPages())
  await first.view.open('moai-panel/overview')
  await first.view.follow({ kind: 'wiki', slug: PLAYGROUND.slug })
  first.view.toggleRaw()
  first.view.setQuery('moai')
  scroller(first.view).scrollTop = 64
  const saved = first.view.state()
  assert.deepEqual({ ...saved, scrollTop: undefined }, {
    current: PLAYGROUND.slug, cursor: 'moai-panel/tui-analysis', trail: [{ slug: 'moai-panel/overview', scrollTop: 0 }],
    query: 'moai', raw: true, scrollTop: undefined, selectedLink: -1, pane: 'list',
  })
  assert.equal(saved.scrollTop, 64)
  const second = makeView()
  second.view.setPages(allPages())
  await tick()
  await second.view.restore(JSON.parse(JSON.stringify(saved)))
  assert.equal(second.view.state().current, PLAYGROUND.slug)
  assert.equal(scroller(second.view).scrollTop, 64)
  assert.ok(scroller(second.view).querySelector('pre.m-wiki-raw'))
  assert.equal(second.view.el.querySelector('.m-wiki-search input').value, 'moai')
  assert.equal(await second.view.back(), true)
  assert.equal(second.view.state().current, 'moai-panel/overview')
})

test('wiki: no innerHTML or followable href; css uses tokens only', () => {
  assert.doesNotMatch(wikiSource, /innerHTML|outerHTML|insertAdjacentHTML|document\.write|\.href\s*=|setAttribute\(\s*["']href/)
  assert.doesNotMatch(wikiSource, /window\.open|location\.(assign|href|replace)/)
  assert.doesNotMatch(wikiCss, /@import|url\(|gradient|box-shadow|text-shadow|blur\(/)
  assert.doesNotMatch(wikiCss.replace(/\/\*[^]*?\*\//g, ''), /#[0-9a-fA-F]{3,8}\b/, 'colours come from tokens')
  // The narrow rule in wiki.css stacks the list over the page.
  assert.match(wikiCss, /data-position="bottom"\] \.m-split-detail \{[^}]*order: 0/)
})

test('wiki: the view, its warnings and its parse errors speak the words they are given (se-ti70)', () => {
  for (const lang of ['en', 'ja', 'zh']) {
    const words = COPY[lang].wiki
    const { view } = makeView({ words })
    assert.equal(view.el.querySelector('.m-wiki-search input').placeholder, words.searchPlaceholder)
    view.setPages([], 'docs')
    assert.ok(view.el.querySelector('.m-wiki-list-note').textContent.includes(words.noPages))
    assert.deepEqual(wiki.pageWarnings({ conflict: true, bytes: 2e6 }, words).map((w) => w.label), [words.conflict, words.large])
    assert.equal(wiki.parseWikiLs({ dir: 'docs' }, words).error, words.listNoPages)
    assert.doesNotMatch(view.el.textContent, /[가-힣]/, `${lang}: no Korean left`)
  }
})
