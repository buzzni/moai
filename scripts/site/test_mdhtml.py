"""Tests for mdhtml.py. Run: python3 -I -B -m unittest discover -s scripts/site"""

import re
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from mdhtml import Heading, github_anchor, render  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]
ALLOWED_TAGS = {
    "h1", "h2", "h3", "h4", "h5", "h6", "p", "ul", "ol", "li", "pre", "code", "div", "table", "thead",
    "tbody", "tr", "th", "td", "a", "strong", "em", "br", "hr", "blockquote", "del", "span",
}


def html(src: str, **kw) -> str:
    return render(src, **kw)[0]


class Anchors(unittest.TestCase):
    def test_rule(self):
        self.assertEqual(github_anchor("  Hello, World! (v2) "), "hello-world-v2")
        self.assertEqual(github_anchor("`moai add` a_b-c"), "moai-add-a_b-c")
        self.assertEqual(github_anchor("Zusätzliche Änderung"), "zusätzliche-änderung")

    def test_dedupe(self):
        seen: dict[str, int] = {}
        got = [github_anchor(t, seen) for t in ("Foo", "Foo", "Foo-1")]
        self.assertEqual(got, ["foo", "foo-1", "foo-1-1"])
        seen = {}
        got = [github_anchor(t, seen) for t in ("Foo", "Foo-1", "Foo")]
        self.assertEqual(got, ["foo", "foo-1", "foo-2"])

    def test_headings_returned(self):
        out, heads = render("# One\n\n## Two `x`\n\n## Two `x`\n")
        self.assertEqual(
            heads, [Heading(1, "One", "one"), Heading(2, "Two x", "two-x"), Heading(2, "Two x", "two-x-1")]
        )
        self.assertIn('<h1 id="one">One</h1>', out)
        self.assertIn('<h2 id="two-x-1">Two <code>x</code></h2>', out)


class Escaping(unittest.TestCase):
    def test_angle_text(self):
        self.assertEqual(html("Use <id> and <epic> here"), "<p>Use &lt;id&gt; and &lt;epic&gt; here</p>")

    def test_in_code(self):
        self.assertIn("<code>&lt;id&gt;</code>", html("run `<id>`"))
        self.assertIn("<pre><code>moai show &lt;id&gt; &amp;&amp; x</code></pre>", html("```\nmoai show <id> && x\n```"))

    def test_raw_html_is_text(self):
        out = html("<script>alert(1)</script>")
        self.assertNotIn("<script", out)
        self.assertIn("&lt;script&gt;", out)

    def test_image_is_alt_text(self):
        out = html("![alt <b>](x.png)")
        self.assertNotIn("<img", out)
        self.assertIn("![alt &lt;b&gt;](x.png)", out)

    def test_backslash_escape(self):
        self.assertEqual(html(r"\*not em\* \<id\>"), "<p>*not em* &lt;id&gt;</p>")


class Blocks(unittest.TestCase):
    @staticmethod
    def task(checked):
        return (f'<span class="task" role="checkbox" data-checked="{checked}" '
                f'aria-checked="{checked}" aria-disabled="true"></span>')

    def test_task_markers(self):
        out = html('- [ ] todo\n- [x] done\n- [X] DONE\n- not [ ] here\n- [ ]\n\ntext [x] y')
        self.assertEqual(re.findall(r'data-checked="(\w+)"', out), ["false", "true", "true"])
        self.assertNotIn("<input", out)
        # whole items: a bare "[ ]" would match inside "not [ ] here" and prove nothing about `- [ ]`
        for literal in ("<li>not [ ] here</li>", "<li>[ ]</li>", "<p>text [x] y</p>"):
            self.assertIn(literal, out)
        self.assertIn(".doc-body .task", (ROOT / "site/assets/site.css").read_text(encoding="utf-8"))

    def test_task_tight_and_loose(self):
        for separator, paragraph in (("\n", False), ("\n\n", True)):
            with self.subTest(paragraph=paragraph):
                items = []
                for checked, text in (("false", "a"), ("true", "b")):
                    content = self.task(checked) + " " + text
                    if paragraph:
                        content = "<p>" + content + "</p>"
                    items.append('<li class="task-item">' + content + '</li>')
                self.assertEqual(html('- [ ] a' + separator + '- [x] b'),
                                 '<ul>\n' + '\n'.join(items) + '\n</ul>')

    def test_task_ordered_nested_and_escaped(self):
        self.assertEqual(html('1. [x] a'), '<ol>\n<li class="task-item">' + self.task("true") + ' a</li>\n</ol>')
        out = html('- [ ] a\n  - [x] b')
        self.assertEqual(out.count('class="task-item"'), 2)
        self.assertIn(self.task("true") + ' b</li>', out)
        self.assertIn(self.task("false") + ' &lt;b&gt;', html('- [ ] <b>'))

    def test_task_block_first_and_empty_items(self):
        for src, block in (('- [ ] # heading', '<h1'), ('- [x] ```\n  code\n  ```', '<pre>')):
            with self.subTest(src=src):
                out = html(src)
                self.assertIn('<li class="task-item">' + self.task("false" if '[ ]' in src else "true") + ' \n' + block, out)
        for src in ('- ', '- [ ]', '- [x] ', r'- \[x] escaped'):
            with self.subTest(src=src):
                self.assertNotIn('class="task"', html(src))

    def test_table_alignment(self):
        src = "| a | b | c |\n|:--|:-:|--:|\n| 1 | 2 | 3 |\n| x | y |\n"
        out = html(src)
        self.assertIn('<div class="table"><table>', out)
        self.assertIn("<th>a</th>", out)
        self.assertIn('<th style="text-align:center">b</th>', out)
        self.assertIn('<th style="text-align:right">c</th>', out)
        self.assertIn('<td style="text-align:right">3</td>', out)
        self.assertIn('<td style="text-align:right"></td>', out)
        self.assertIn("<thead><tr>", out)
        self.assertIn("<tbody>", out)

    def test_nested_list_with_indented_code(self):
        src = "- item one\n\n  more text\n\n      code <x>\n      line2\n\n  - nested\n- two\n"
        out = html(src)
        self.assertIn("<pre><code>code &lt;x&gt;\nline2</code></pre>", out)
        self.assertIn("<ul>\n<li>nested</li>\n</ul>", out)

    def test_tight_list(self):
        self.assertEqual(html("- a\n- b\n"), "<ul>\n<li>a</li>\n<li>b</li>\n</ul>")

    def test_loose_list(self):
        self.assertEqual(html("- a\n\n- b\n"), "<ul>\n<li><p>a</p></li>\n<li><p>b</p></li>\n</ul>")

    def test_tight_item_with_nested(self):
        out = html("- a\n  - b\n")
        self.assertEqual(out, "<ul>\n<li>a\n<ul>\n<li>b</li>\n</ul></li>\n</ul>")

    def test_ordered(self):
        self.assertEqual(html("1. a\n2. b"), "<ol>\n<li>a</li>\n<li>b</li>\n</ol>")
        self.assertIn('<ol start="3">', html("3. a"))

    def test_continuation_line(self):
        self.assertEqual(html("- a\ncont\n- b"), "<ul>\n<li>a\ncont</li>\n<li>b</li>\n</ul>")

    def test_fence_info(self):
        out = html("```sh\nmoai status\n```\n\n~~~\nplain\n~~~")
        self.assertIn('<pre><code class="language-sh">moai status</code></pre>', out)
        self.assertIn("<pre><code>plain</code></pre>", out)

    def test_indented_code(self):
        self.assertEqual(html("text\n\n    a\n\n    b\n\nafter"), "<p>text</p>\n<pre><code>a\n\nb</code></pre>\n<p>after</p>")

    def test_blocks_on_own_lines(self):
        self.assertEqual(html("# T\n\npara\n\n---\n\n> q"), '<h1 id="t">T</h1>\n<p>para</p>\n<hr>\n<blockquote>\n<p>q</p>\n</blockquote>')


class Inline(unittest.TestCase):
    def test_strong_with_code(self):
        self.assertEqual(html("**a `b` c**"), "<p><strong>a <code>b</code> c</strong></p>")

    def test_code_wins_over_emphasis(self):
        self.assertEqual(html("`*a*` *b*"), "<p><code>*a*</code> <em>b</em></p>")

    def test_em_variants(self):
        self.assertEqual(html("*a* _b_ snake_case_name"), "<p><em>a</em> <em>b</em> snake_case_name</p>")

    def test_strike(self):
        self.assertIn("<del>x</del>", html("~~x~~"))

    def test_links(self):
        self.assertEqual(html("[a](b.md#c)"), '<p><a href="b.md#c">a</a></p>')
        self.assertEqual(
            html('[x](https://e.com/?a=1&b="2")'),
            '<p><a href="https://e.com/?a=1&amp;b=&quot;2&quot;" rel="noopener">x</a></p>',
        )
        self.assertEqual(html("[m](mailto:a@b.c)"), '<p><a href="mailto:a@b.c">m</a></p>')

    def test_resolver_gets_raw_href(self):
        seen: list[str] = []

        def res(h: str) -> str:
            seen.append(h)
            return "/new/" + h

        out = html("[a](workflow.md#x) [b](https://x.y)", resolve_link=res)
        self.assertEqual(seen, ["workflow.md#x", "https://x.y"])
        self.assertIn('<a href="/new/workflow.md#x">a</a>', out)

    def test_javascript_refused(self):
        for bad in ("javascript:alert(1)", "JaVaScRiPt:alert(1)", "data:text/html,x", "vbscript:x"):
            out = html(f"[click]({bad})")
            self.assertNotIn("<a", out, bad)
            self.assertNotIn("href", out, bad)
            self.assertIn("click", out)
        self.assertNotIn("<a", html("[x](java&#9;script:alert(1))"))
        self.assertNotIn("<a", html("<javascript:alert(1)>"))

    def test_resolver_not_called_for_refused(self):
        called: list[str] = []
        html("[x](javascript:1)", resolve_link=lambda h: called.append(h) or h)
        self.assertEqual(called, [])

    def test_resolver_output_checked(self):
        self.assertNotIn("<a", html("[x](a.md)", resolve_link=lambda h: "javascript:1"))

    def test_autolink(self):
        self.assertIn('<a href="https://e.com" rel="noopener">https://e.com</a>', html("<https://e.com>"))

    def test_entities(self):
        self.assertEqual(html("a &amp; b &lt;x&gt; &bogus;"), "<p>a &amp; b &lt;x&gt; &amp;bogus;</p>")

    def test_hard_break(self):
        self.assertEqual(html("a  \nb"), "<p>a<br>b</p>")

    def test_ref_links(self):
        self.assertIn('<a href="http://x" rel="noopener">t</a>', html("[t][r]\n\n[r]: http://x"))

    def test_deterministic(self):
        src = "# A\n\n- x *y*\n\n| a | b |\n|---|---|\n| 1 | 2 |\n"
        self.assertEqual(render(src), render(src))


class Smoke(unittest.TestCase):
    def test_all_pages(self):
        pages = sorted((ROOT / "docs").rglob("*.md")) + [ROOT / "CHANGELOG.md"]
        self.assertGreater(len(pages), 3)
        for page in pages:
            with self.subTest(page=str(page.relative_to(ROOT))):
                out, heads = render(page.read_text(encoding="utf-8"))
                tags = set(re.findall(r"<\s*/?\s*([A-Za-z][A-Za-z0-9]*)", out))
                self.assertLessEqual(tags, ALLOWED_TAGS, tags - ALLOWED_TAGS)
                self.assertTrue(out.strip())
                for h in heads:
                    self.assertIn(f'id="{h.anchor}"', out)
                # every heading has a unique id
                anchors = [h.anchor for h in heads]
                self.assertEqual(len(anchors), len(set(anchors)))


if __name__ == "__main__":
    unittest.main()
