// Moai panel markdown renderer: supported constructs, GitHub anchors, links without href, and XSS safety.
import assert from 'node:assert/strict'
import { readFile, readdir } from 'node:fs/promises'
import { test } from 'node:test'
import { JSDOM } from 'jsdom'

const panel = new URL('../panel/', import.meta.url)
const { renderMarkdown, githubAnchor } = await import(new URL('view/markdown.ts', panel))
const sourceText = await readFile(new URL('view/markdown.ts', panel), 'utf8')
const css = await readFile(new URL('style/markdown.css', panel), 'utf8')

const dom = new JSDOM('<!doctype html><body></body>')
const doc = dom.window.document
const md = (source, options) => renderMarkdown(doc, source, options)
const html = (source, options) => md(source, options).root.innerHTML

/** The safety invariant, over a whole rendered tree. */
function assertSafe(root) {
  for (const node of [root, ...root.querySelectorAll('*')]) {
    for (const attr of node.attributes) {
      assert.ok(!/^(href|src|srcset|action|formaction|style|xlink:href)$/i.test(attr.name), `${node.tagName} has ${attr.name}`)
      assert.ok(!/^on/i.test(attr.name), `${node.tagName} has ${attr.name}`)
    }
  }
  assert.equal(root.querySelectorAll('script, img, iframe, style, object, embed, link, svg, form, input').length, 0)
}

test('source: no innerHTML, outerHTML, insertAdjacentHTML or document.write', () => {
  assert.doesNotMatch(sourceText, /innerHTML|outerHTML|insertAdjacentHTML|document\.write|DOMParser|createContextualFragment/)
})

test('css: tokens only, no url, import, gradient or shadow', () => {
  assert.match(css, /\.m-md\b/)
  assert.doesNotMatch(css, /url\(|@import|gradient|shadow/)
})

test('headings: levels, ids, data-anchor, list in order, repeats', () => {
  const r = md('# Title\n\n## 한글 제목!\n\n### Foo\n\n### Foo\n\n#### Foo ####\n\n#nospace\n')
  assert.deepEqual(r.headings.map((h) => [h.level, h.text, h.anchor]), [
    [1, 'Title', 'title'],
    [2, '한글 제목!', '한글-제목'],
    [3, 'Foo', 'foo'],
    [3, 'Foo', 'foo-1'],
    [4, 'Foo', 'foo-2'],
  ])
  const h = r.root.querySelector('h2')
  assert.equal(h.id, 'm-md-한글-제목')
  assert.equal(h.getAttribute('data-anchor'), '한글-제목')
  assert.equal(r.root.querySelector('h4').id, 'm-md-foo-2')
  assert.equal(r.root.className, 'm-md')
  assert.match(r.root.textContent, /#nospace/)
})

test('githubAnchor: Korean kept, punctuation dropped, spaces to dashes, repeats numbered', () => {
  assert.equal(githubAnchor('Hello, World!'), 'hello-world')
  assert.equal(githubAnchor('한글 제목 (설명)'), '한글-제목-설명')
  assert.equal(githubAnchor('a_b-c d'), 'a_b-c-d')
  assert.equal(githubAnchor('v1.2 API/Keys'), 'v12-apikeys')
  assert.equal(githubAnchor('A  B'), 'a--b')
  const seen = new Map()
  assert.deepEqual([githubAnchor('foo', seen), githubAnchor('foo', seen), githubAnchor('Foo', seen), githubAnchor('bar', seen)], ['foo', 'foo-1', 'foo-2', 'bar'])
  // A numbered slug already taken by a heading is counted again (github-slugger, moai wiki::Anchors).
  const taken = new Map()
  assert.deepEqual(['Foo', 'Foo-1', 'Foo', 'Foo'].map((h) => githubAnchor(h, taken)), ['foo', 'foo-1', 'foo-2', 'foo-3'])
  // GitHub drops superscript and circled digits but keeps combining marks.
  assert.equal(githubAnchor('x² ①'), 'x-')
  assert.equal(githubAnchor('Café'), 'café')
})

test('headings: a taken numbered anchor never repeats; setext headings; a leading BOM', () => {
  const r = md('# Foo\n\n# Foo-1\n\n# Foo\n')
  assert.deepEqual(r.headings.map((h) => h.anchor), ['foo', 'foo-1', 'foo-2'])
  assert.equal(new Set([...r.root.querySelectorAll('[id]')].map((h) => h.id)).size, 3, 'ids are unique')
  const setext = md('Title\n=====\n\nSub *x*\n---\n\ntext')
  assert.deepEqual(setext.headings.map((h) => [h.level, h.text, h.anchor]), [[1, 'Title', 'title'], [2, 'Sub x', 'sub-x']])
  assert.equal(setext.root.querySelector('hr'), null)
  assert.equal(md('﻿# Title\n\nx').headings[0]?.text, 'Title')
})

test('paragraphs, soft and hard breaks, escapes', () => {
  assert.equal(html('one\ntwo\n\nthree'), '<p>one\ntwo</p><p>three</p>')
  assert.equal(html('a  \nb'), '<p>a<br>b</p>')
  assert.equal(html('a\\\nb'), '<p>a<br>b</p>')
  assert.equal(html('\\*not em\\* \\# \\[x\\]'), '<p>*not em* # [x]</p>')
})

test('inline: bold, italic, code, nesting', () => {
  assert.equal(html('**b** __b2__ *i* _i2_'), '<p><strong>b</strong> <strong>b2</strong> <em>i</em> <em>i2</em></p>')
  assert.equal(html('***both***'), '<p><strong><em>both</em></strong></p>')
  assert.equal(html('*a **b** c*'), '<p><em>a <strong>b</strong> c</em></p>')
  assert.equal(html('snake_case_word and 2*3*4'), '<p>snake_case_word and 2<em>3</em>4</p>')
  assert.equal(html('a * b * c'), '<p>a * b * c</p>')
  assert.equal(html('`x *y*` and ``a ` b``'), '<p><code class="m-md-inline-code">x *y*</code> and <code class="m-md-inline-code">a ` b</code></p>')
  assert.equal(html('unclosed `tick and **bold'), '<p>unclosed `tick and **bold</p>')
})

test('inline: openers that never close stay fast (no exponential backtracking)', () => {
  // 40 unclosed `*x` openers took minutes before the per-position memo; a glob list is the everyday shape.
  const started = Date.now()
  const r = md('*a '.repeat(40) + '\n\nWatched: ' + Array.from({ length: 40 }, (_, i) => `*.e${i}`).join(', '))
  assert.ok(Date.now() - started < 2000, `took ${Date.now() - started}ms`)
  assert.equal(r.root.querySelector('em'), null)
  assert.match(r.root.textContent, /^\*a \*a /)
})

test('code blocks: fenced with ``` and ~~~, indented, nothing parsed inside', () => {
  const r = md('```js\nconst a = "<b>"; *x*\n```\n\n~~~\ntilde\n~~~\n\n    indented\n    code\n\nafter')
  const pres = [...r.root.querySelectorAll('pre.m-md-code')]
  assert.equal(pres.length, 3)
  assert.equal(pres[0].textContent, 'const a = "<b>"; *x*')
  assert.equal(pres[0].getAttribute('data-lang'), 'js')
  assert.equal(pres[1].textContent, 'tilde')
  assert.equal(pres[2].textContent, 'indented\ncode')
  assert.equal(r.root.querySelector('b'), null)
  assert.equal(r.root.querySelector('pre em'), null)
  assert.equal(md('```\nunclosed\nstill code').root.querySelector('pre').textContent, 'unclosed\nstill code')
  assert.equal(md('````\n```\ninner\n```\n````').root.querySelector('pre').textContent, '```\ninner\n```')
})

test('block quotes, thematic breaks', () => {
  const r = md('> quoted **x**\n> more\n>\n> > nested\n\n---\n\n***\n\n___')
  assert.equal(r.root.querySelectorAll('blockquote').length, 2)
  assert.equal(r.root.querySelector('blockquote strong').textContent, 'x')
  assert.equal(r.root.querySelector('blockquote blockquote p').textContent, 'nested')
  assert.equal(r.root.querySelectorAll('hr').length, 3)
})

test('lists: bullets, ordered with start, nested, tight vs loose', () => {
  const r = md('- a\n- b\n  - b1\n  - b2\n    1. deep\n    2. deeper\n- c\n\n3. three\n4. four\n')
  const top = r.root.querySelector('ul')
  const items = [...top.children].map((li) => li.tagName)
  assert.deepEqual(items, ['LI', 'LI', 'LI'])
  assert.equal(top.children[0].innerHTML, 'a')
  assert.equal(top.children[1].querySelector(':scope > ul').children.length, 2)
  const deep = top.children[1].querySelector('ul ul ~ *, ul li ol') ?? top.querySelector('ol')
  assert.deepEqual([...top.querySelector('ol').children].map((li) => li.textContent), ['deep', 'deeper'])
  assert.ok(deep)
  const ordered = r.root.querySelector(':scope > ol')
  assert.equal(ordered.getAttribute('start'), '3')
  assert.equal(ordered.children.length, 2)
  // Loose list wraps items in paragraphs.
  assert.equal(html('- a\n\n- b'), '<ul><li><p>a</p></li><li><p>b</p></li></ul>')
  assert.equal(html('1. a\n2. b'), '<ol><li>a</li><li>b</li></ol>')
  // Lazy continuation and an item holding a code block.
  assert.equal(html('- one\ntwo'), '<ul><li>one\ntwo</li></ul>')
  const withCode = md('- step\n\n  ```\n  code\n  ```\n- next')
  assert.equal(withCode.root.querySelector('li pre').textContent, 'code')
  // A dash line that is a rule, not an item.
  assert.equal(md('- a\n\n---\n\n- b').root.querySelectorAll('hr').length, 1)
  // A list can interrupt a paragraph.
  assert.equal(md('text\n- a\n- b').root.querySelectorAll('li').length, 2)
  // A blank line before a nested list keeps the list tight, but two paragraphs never run together.
  assert.equal(html('- first\n\n  second\n\n  - sub'), '<ul><li>first<p>second</p><ul><li>sub</li></ul></li></ul>')
})

test('tables: header, body, alignment, escaped pipes, inline content, ragged rows', () => {
  const r = md('| Name | Qty | Note |\n|:-----|----:|:----:|\n| `a\\|b` | 1 | **x** |\n| c \\| d | 2 |\n\nafter')
  const table = r.root.querySelector('table')
  assert.ok(table)
  const ths = [...table.querySelectorAll('thead th')]
  assert.deepEqual(ths.map((t) => t.textContent), ['Name', 'Qty', 'Note'])
  assert.deepEqual(ths.map((t) => t.getAttribute('data-align')), ['left', 'right', 'center'])
  const rows = [...table.querySelectorAll('tbody tr')]
  assert.equal(rows.length, 2)
  assert.deepEqual([...rows[1].children].map((c) => c.textContent), ['c | d', '2', ''])
  assert.equal(rows[0].querySelector('strong').textContent, 'x')
  assert.equal(rows[0].querySelector('code').textContent, 'a|b')
  assert.equal(rows[0].children[1].getAttribute('data-align'), 'right')
  assert.equal(r.root.querySelector(':scope > p').textContent, 'after')
  // No leading/trailing pipes, and plain alignment-less columns.
  const bare = md('a | b\n--- | ---\n1 | 2')
  assert.equal(bare.root.querySelectorAll('td').length, 2)
  assert.equal(bare.root.querySelector('th').hasAttribute('data-align'), false)
  // Not a table: no delimiter row.
  assert.equal(md('a | b\nc | d').root.querySelector('table'), null)
})

test('links: markdown links, titles, autolinks, default resolution, no href ever', () => {
  const r = md('[ext](https://example.com/a?b=1 "Title") [anc](#Hello%20World) <https://auto.example/x> [**bold** `c`](rel.md)')
  assert.equal(r.links.length, 4)
  const [ext, anc, auto, rel] = r.links
  assert.equal(ext.getAttribute('data-kind'), 'external')
  assert.equal(ext.getAttribute('data-href'), 'https://example.com/a?b=1')
  assert.equal(ext.getAttribute('title'), 'https://example.com/a?b=1')
  assert.equal(ext.textContent, 'ext')
  assert.equal(anc.getAttribute('data-kind'), 'anchor')
  assert.equal(anc.getAttribute('data-anchor'), 'hello-world')
  assert.equal(auto.getAttribute('data-href'), 'https://auto.example/x')
  assert.equal(auto.textContent, 'https://auto.example/x')
  assert.equal(rel.querySelector('strong').textContent, 'bold')
  for (const a of r.links) {
    assert.equal(a.tagName, 'A')
    assert.equal(a.getAttribute('class'), 'm-md-link')
    assert.equal(a.getAttribute('tabindex'), '-1')
    assert.equal(a.hasAttribute('href'), false)
  }
  assertSafe(r.root)
  // Parentheses in the destination and angle-bracket destinations.
  assert.equal(md('[w](https://x.example/a_(b))').links[0].getAttribute('data-href'), 'https://x.example/a_(b)')
  assert.equal(md('[w](<a b.md>)').links[0].getAttribute('data-href'), 'a b.md')
  // Broken syntax stays text.
  assert.equal(html('[no close(x) and [a]'), '<p>[no close(x) and [a]</p>')
  // A link inside a link is not nested.
  assert.equal(md('[a [b](x.md) c](y.md)').root.querySelectorAll('a a').length, 0)
})

test('resolveLink: the callback decides the target and is honoured for every link', () => {
  const calls = []
  const r = md('[w](a/b.md#sec) [x](#top) [e](http://e.example) <mailto:a@b.c> [bad](zzz)', {
    resolveLink: (href) => {
      calls.push(href)
      if (href === 'a/b.md#sec') return { kind: 'wiki', slug: 'a/b', anchor: 'sec' }
      if (href === '#top') return { kind: 'anchor', anchor: 'top' }
      if (href === 'zzz') throw new Error('boom')
      return { kind: 'external', href }
    },
  })
  assert.deepEqual(calls, ['a/b.md#sec', '#top', 'http://e.example', 'mailto:a@b.c', 'zzz'])
  const [w, x, e, m, bad] = r.links
  assert.deepEqual([w.dataset.kind, w.dataset.slug, w.dataset.anchor], ['wiki', 'a/b', 'sec'])
  assert.deepEqual([x.dataset.kind, x.dataset.anchor], ['anchor', 'top'])
  assert.deepEqual([e.dataset.kind, e.dataset.href], ['external', 'http://e.example'])
  assert.equal(m.dataset.href, 'mailto:a@b.c')
  assert.equal(bad.dataset.kind, 'external')
  assert.equal(w.hasAttribute('title'), false)
  assertSafe(r.root)
})

test('links: collected in document order across blocks', () => {
  const r = md('# [h](h.md)\n\n- [l1](1.md)\n- [l2](2.md)\n\n| a |\n|---|\n| [t](3.md) |\n\n> [q](4.md)\n\nse-1', { issueIds: ['se-1'] })
  assert.deepEqual(r.links.map((a) => a.textContent), ['h', 'l1', 'l2', 't', 'q', 'se-1'])
  assert.deepEqual(r.links, [...r.root.querySelectorAll('a')])
})

test('issue ids: whole words only, not in code, not inside links', () => {
  const ids = ['se-abc', 'se-abc.def', 'se-x']
  const r = md('See se-abc and se-abc.def, also (se-x). Not xse-abc, se-abcd, se-abc-1, se-abc.defg, se-x_y.\n\n`se-abc` and\n\n```\nse-abc\n```\n\n[se-abc](a.md) <https://e.example/se-abc> and **se-x**', { issueIds: ids })
  const issues = [...r.root.querySelectorAll('a[data-kind="issue"]')]
  assert.deepEqual(issues.map((a) => a.dataset.issue), ['se-abc', 'se-abc.def', 'se-x', 'se-abc', 'se-x'])
  assert.deepEqual(issues.map((a) => a.className), Array(5).fill('m-md-link m-md-issue'))
  for (const a of issues) { assert.equal(a.getAttribute('tabindex'), '-1'); assert.equal(a.hasAttribute('href'), false) }
  assert.equal(r.root.querySelector('code a'), null, 'the link wraps the code, never the reverse')
  assert.equal(r.root.querySelector('pre a'), null)
  assert.equal(r.root.querySelectorAll('a a').length, 0)
  const md1 = [...r.root.querySelectorAll('a[data-kind="external"]')]
  assert.equal(md1[0].textContent, 'se-abc')
  assert.equal(md1[0].querySelector('a'), null)
  // No issue ids means no issue links.
  assert.equal(md('se-abc').root.querySelector('a'), null)
  // Regex metacharacters in ids are escaped.
  assert.equal(md('a.b and axb', { issueIds: ['a.b'] }).links.length, 1)
})

test('issue ids in code spans: only a span that is exactly an id links, wrapping the code', () => {
  const r = md('`se-1` and ` se-2 ` and `se-1 more` and `x se-1` and [`se-1`](a.md) and **`se-1`**\n\n```\nse-1\n```\n\n    se-1', { issueIds: ['se-1', 'se-2'] })
  const issues = [...r.root.querySelectorAll('a[data-kind="issue"]')]
  assert.deepEqual(issues.map((a) => a.dataset.issue), ['se-1', 'se-2', 'se-1'])
  for (const a of issues) {
    assert.equal(a.className, 'm-md-link m-md-issue')
    assert.equal(a.getAttribute('tabindex'), '-1')
    assert.equal(a.hasAttribute('href'), false)
    assert.equal(a.children.length, 1)
    assert.equal(a.firstElementChild.className, 'm-md-inline-code')
  }
  assert.equal(r.root.querySelectorAll('a a').length, 0)
  assert.equal(r.root.querySelectorAll('pre a').length, 0)
  // Inside a markdown link it stays plain code.
  const wrapped = r.root.querySelector('a[data-kind="external"]')
  assert.equal(wrapped.querySelector('code').textContent, 'se-1')
  assert.equal(wrapped.querySelector('a'), null)
  // Mixed text stays plain, and without issueIds nothing links.
  assert.equal(md('`se-1 more`', { issueIds: ['se-1'] }).links.length, 0)
  assert.equal(md('`se-1`').links.length, 0)
  assertSafe(r.root)
})

test('XSS: raw HTML renders as literal text and creates no element', () => {
  const source = [
    '<script>alert(1)</script>',
    '',
    '<img src=x onerror=alert(1)>',
    '',
    '<a href="javascript:alert(1)">click</a>',
    '',
    'inline <b onclick="x()">bold</b> and <iframe src="//evil"></iframe> <style>*{}</style>',
    '',
    '- <script>x</script>',
    '',
    '| <img src=x onerror=y> |\n|---|\n| <svg onload=z> |',
    '',
    '> <object data=x>',
    '',
    '# <img src=x onerror=h>',
  ].join('\n')
  const r = md(source)
  assertSafe(r.root)
  assert.equal(r.root.querySelectorAll('a').length, 0)
  const text = r.root.textContent
  for (const literal of ['<script>alert(1)</script>', '<img src=x onerror=alert(1)>', '<a href="javascript:alert(1)">click</a>', '<b onclick="x()">', '<iframe src="//evil"></iframe>', '<style>*{}</style>', '<svg onload=z>', '<object data=x>']) {
    assert.ok(text.includes(literal), `literal ${literal}`)
  }
  assert.equal(r.headings[0].text, '<img src=x onerror=h>')
})

test('XSS: dangerous link destinations give an <a> with no href and no handlers', () => {
  const cases = [
    '[x](javascript:alert(1))',
    '[x](JaVaScRiPt:alert(1))',
    '[x](data:text/html;base64,PHNjcmlwdD4=)',
    '[x](vbscript:msgbox)',
    '[x](<javascript:alert(1)>)',
    '<javascript:alert(1)>',
    '<data:text/html,x>',
    '[x](&#106;avascript:alert(1))',
    '[x](  javascript:alert(1) "t")',
    '[x]("onmouseover="alert(1))',
  ]
  for (const source of cases) {
    const r = md(source)
    assertSafe(r.root)
    assert.equal(r.root.querySelectorAll('a').length >= 1 || source.includes('onmouseover'), true, source)
    for (const a of r.root.querySelectorAll('a')) {
      assert.equal(a.getAttribute('data-kind'), 'external', source)
      assert.equal(a.hasAttribute('href'), false)
      assert.equal([...a.attributes].some((attr) => /^on/i.test(attr.name)), false)
    }
  }
  const js = md('[x](javascript:alert(1))').links[0]
  assert.equal(js.dataset.href, 'javascript:alert(1)')
  assert.equal(js.textContent, 'x')
  assert.equal(md('<javascript:alert(1)>').links.length, 1)
  // Even a hostile resolveLink cannot put an href or handler on the element.
  const hostile = md('[x](a)', { resolveLink: () => ({ kind: 'external', href: 'javascript:1', onclick: 'x()', extra: 1 }) })
  assertSafe(hostile.root)
})

test('XSS: images are never created', () => {
  for (const source of ['![a](http://x/p.png)', '![](x.png)', '![a](javascript:alert(1))', '![a][ref]\n\n[ref]: http://x/y.png', '[![a](x.png)](y.md)', '<img src="http://x/p.png">']) {
    const r = md(source)
    assert.equal(r.root.querySelectorAll('img').length, 0, source)
    assertSafe(r.root)
  }
  assert.equal(md('![a](http://x/p.png)').root.textContent, '![a](http://x/p.png)')
})

test('XSS: attribute and tag injection through headings, link text, issue ids and code info', () => {
  const r = md('# " onmouseover="x"\n\n[" onfocus="y](a.md "t\\" onclick=\\"z")\n\n```" onload="q\ncode\n```\n\nse-1"onclick=', { issueIds: ['se-1'] })
  assertSafe(r.root)
  assert.equal(r.root.querySelector('pre').getAttribute('data-lang'), '"')
})

test('smoke: real wiki pages render with tables and links, safely', async () => {
  const dir = new URL('./fixtures/moai-wiki/', import.meta.url)
  const files = (await readdir(dir)).filter((f) => f.startsWith('wiki-show-'))
  assert.ok(files.length >= 3)
  let tables = 0
  let anyLinks = 0
  for (const file of files) {
    const page = JSON.parse(await readFile(new URL(file, dir), 'utf8'))
    const r = md(page.body, { issueIds: (page.issues ?? []).map((i) => i.id) })
    assertSafe(r.root)
    assert.ok(r.headings.length > 0, file)
    assert.ok(r.root.textContent.length > page.body.length / 4, file)
    assert.equal(r.root.querySelectorAll('p').length > 0, true, file)
    assert.equal(new Set(r.headings.map((h) => h.anchor)).size, r.headings.length, `unique anchors in ${file}`)
    // No markdown syntax leaks through as literal text.
    assert.doesNotMatch(r.root.textContent, /\*\*[^*\s][^*]*\*\*/, `leftover bold in ${file}`)
    assert.doesNotMatch(r.root.textContent, /^\|[-: |]+\|$/m, `leftover table rule in ${file}`)
    assert.doesNotMatch(r.root.textContent, /^#{1,6} /m, `leftover heading in ${file}`)
    tables += r.root.querySelectorAll('table').length
    anyLinks += r.links.length
    if (file === 'wiki-show-overview.json') {
      assert.ok(r.root.querySelectorAll('table').length > 0)
      assert.ok(r.links.length > 0)
      // The fixture names issue ids inside code spans; those become issue links around the code.
      const issues = r.links.filter((a) => a.dataset.kind === 'issue')
      assert.ok(issues.length > 0)
      assert.ok(issues.every((a) => a.firstElementChild?.tagName === 'CODE' && a.textContent === a.dataset.issue))
      // Cells hold their links.
      assert.ok(r.root.querySelector('td a, th a'))
    }
  }
  assert.ok(tables > 0)
  assert.ok(anyLinks > 0)
})

const ext = (href) => ({ kind: 'external', href })

test('entities: named and numeric decode into text, never markup', () => {
  assert.equal(md('a &amp; b &lt;script&gt; &copy; &mdash; &#65;&#x42; &nbsp;|').root.textContent, 'a & b <script> \u00a9 \u2014 AB \u00a0|')
  const r = md('&lt;script&gt;alert(1)&lt;/script&gt; &lt;b&gt;x&lt;/b&gt;')
  assert.equal(r.root.querySelector('script, b'), null)
  assert.equal(r.root.textContent, '<script>alert(1)</script> <b>x</b>')
  assertSafe(r.root)
  // Bad numerics and unknown names stay literal; decoding is single-pass.
  assert.equal(md('&#0; &#xD800; &#1114112; &#7; &bogus; &amp;lt;').root.textContent, '&#0; &#xD800; &#1114112; &#7; &bogus; &lt;')
})

test('entities: not decoded in code spans or fences; decoded in link text and destination', () => {
  const r = md('`&amp;` and\n\n```\n&lt; &amp;\n```\n\n[a &amp; b](x?a=1&amp;b=2)', { resolveLink: ext })
  assert.equal(r.root.querySelector('code.m-md-inline-code').textContent, '&amp;')
  assert.equal(r.root.querySelector('pre').textContent, '&lt; &amp;')
  assert.equal(r.links[0].textContent, 'a & b')
  assert.equal(r.links[0].getAttribute('data-href'), 'x?a=1&b=2')
})

test('inline: thousands of unclosed openers render in well under a second', () => {
  for (const unit of ['*a ', '_a ', '*a **b ']) {
    const started = Date.now()
    const r = md(unit.repeat(5000))
    assert.ok(Date.now() - started < 2000, `${JSON.stringify(unit)} took ${Date.now() - started}ms`)
    assert.equal(r.root.querySelector('em, strong'), null)
  }
  assert.equal(html('*a* **b** and *c*'), '<p><em>a</em> <strong>b</strong> and <em>c</em></p>')
})

test('block quotes: a fence opened inside a quote ends with the quote', () => {
  const r = md('> ```\n> code\nplain text')
  assert.equal(r.root.querySelector('pre').textContent, 'code')
  assert.equal(r.root.querySelector('blockquote pre').parentElement.tagName, 'BLOCKQUOTE')
  assert.equal(r.root.querySelector(':scope > p').textContent, 'plain text')
  // Lazy continuation of an ordinary quoted paragraph is unchanged.
  assert.equal(html('> a\nb'), '<blockquote><p>a\nb</p></blockquote>')
})

test('links: destinations with spaces', () => {
  const seen = []
  const resolveLink = (h) => (seen.push(h), ext(h))
  const r = md('[t](a b.md) and [u](<a b.md>)', { resolveLink })
  assert.deepEqual(seen, ['a b.md'])
  assert.equal(r.links.length, 1)
  assert.equal(r.links[0].textContent, 'u')
  assert.equal(r.root.textContent, '[t](a b.md) and u')
})

test('links: reference style, shortcut, collapsed, case-insensitive, definitions removed', () => {
  const src = '[text][Ref] [ref][] [REF] [missing] [x][nope]\n\n[ref]: http://a.test "Title"\n\nafter\n[other]: <b c.md>\n\n```\n[ref]: not-a-def\n```\n[other]'
  const r = md(src, { resolveLink: ext })
  assert.deepEqual(r.links.map((a) => [a.textContent, a.getAttribute('data-href')]), [
    ['text', 'http://a.test'], ['ref', 'http://a.test'], ['REF', 'http://a.test'], ['other', 'b c.md'],
  ])
  assert.ok(!r.root.textContent.includes('http://a.test "Title"'))
  assert.match(r.root.textContent, /\[missing\] \[x\]\[nope\]/)
  assert.equal(r.root.querySelector('pre').textContent, '[ref]: not-a-def')
  assertSafe(r.root)
})

test('strikethrough: ~~x~~ is <del>, single ~ stays literal', () => {
  assert.equal(html('a ~~gone~~ b ~one~ ~~ x ~~'), '<p>a <del>gone</del> b ~one~ ~~ x ~~</p>')
  assert.equal(html('~~**b**~~'), '<p><del><strong>b</strong></del></p>')
  assert.equal(html('`~~x~~`'), '<p><code class="m-md-inline-code">~~x~~</code></p>')
  assert.match(css, /\.m-md del/)
})

test('task lists: marker span at the start of an item, no input', () => {
  const r = md('- [ ] todo\n- [x] done\n- [X] DONE\n- not [ ] here\n- [ ]\n\ntext [x] y')
  const marks = [...r.root.querySelectorAll('span.m-md-task')]
  assert.deepEqual(marks.map((m) => m.getAttribute('data-checked')), ['false', 'true', 'true'])
  assert.equal(r.root.querySelectorAll('input').length, 0)
  const items = [...r.root.querySelectorAll('li')]
  assert.equal(items[0].textContent.trim(), 'todo')
  assert.equal(items[3].textContent, 'not [ ] here')
  assert.equal(items[4].textContent, '[ ]')
  assert.match(r.root.querySelector('p:last-child').textContent, /text \[x\] y/)
  assertSafe(r.root)
  assert.match(css, /\.m-md-task/)
})
