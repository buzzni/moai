"""Tests for gen.py — run with `python3 -I -B -m unittest discover -s scripts/site`."""

from __future__ import annotations

import re
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import gen  # noqa: E402


def keys(node: dict, prefix: str = "") -> set[str]:
    out = set()
    for k, v in node.items():
        name = f"{prefix}{k}"
        if isinstance(v, dict):
            out |= keys(v, name + ".")
        elif isinstance(v, list):
            for i, item in enumerate(v):
                out |= keys(item, f"{name}[{i}].") if isinstance(item, dict) else {f"{name}[{i}]"}
        else:
            out.add(name)
    return out


class Bundles(unittest.TestCase):
    def test_every_language_carries_the_same_keys(self):
        # A key one language lacks stops the build; a key only one language has is text nobody shows.
        en = keys(gen.load_bundle("en"))
        for lang in gen.LANGS[1:]:
            other = keys(gen.load_bundle(lang))
            self.assertEqual(sorted(en - other), [], f"{lang} lacks these")
            self.assertEqual(sorted(other - en), [], f"only {lang} has these")

    def test_every_key_a_bundle_holds_is_used(self):
        used = set()
        for name in ("layout.html", "home.html", "guide.html"):
            used |= set(gen.SLOT.findall((gen.HERE / name).read_text(encoding="utf-8")))
        computed = {"rotator", "nav", "home.rotator"}  # read by gen.py itself
        unused = [
            k
            for k in keys(gen.load_bundle("en"))
            if k not in used
            and not any(k.startswith(c + ".") or k.startswith(c + "[") or k == c for c in computed)
            and not k.endswith((".title", ".description"))
        ]
        self.assertEqual(unused, [])


class Fill(unittest.TestCase):
    def test_a_missing_key_stops_the_build(self):
        with self.assertRaises(gen.BuildError):
            gen.fill("<p>{{home.nope}}</p>", {}, {"home": {}}, "en")

    def test_a_line_holding_only_an_empty_slot_goes_away(self):
        self.assertEqual(gen.fill("a\n{{x}}\nb", {"x": ""}, {}, "en"), "a\nb")
        self.assertEqual(gen.fill("a {{x}} b", {"x": ""}, {}, "en"), "a  b")

    def test_computed_values_win_over_the_bundle(self):
        self.assertEqual(gen.fill("{{root}}x", {"root": "../"}, {"root": "no"}, "en"), "../x")


class Paths(unittest.TestCase):
    def test_rel(self):
        self.assertEqual(gen.rel("", ""), "./")
        self.assertEqual(gen.rel("", "ko/"), "ko/")
        self.assertEqual(gen.rel("ko/", "ja/"), "../ja/")
        self.assertEqual(gen.rel("ko/guide/", "ko/"), "../")
        self.assertEqual(gen.rel("ko/guide/", "guide/"), "../../guide/")

    def test_a_link_into_the_english_home_keeps_english(self):
        self.assertEqual(gen.page_href("ko/", ""), "../?lang=en")
        self.assertEqual(gen.page_href("guide/", ""), "../?lang=en")
        self.assertEqual(gen.page_href("", "guide/"), "guide/")


class Rotator(unittest.TestCase):
    def test_glint_counts_wide_characters_twice(self):
        self.assertEqual(gen.glint_ms("ab"), (2 + 11) * 120)
        self.assertEqual(gen.glint_ms("가a"), (3 + 11) * 120)
        self.assertEqual(gen.glint_ms("a<br><em>b</em>"), (2 + 11) * 120)


class Docs(unittest.TestCase):
    def link(self, href, here="docs/workflow/"):
        return gen.resolve_doc_link(href, here=here, slugs={"workflow", "cli", "glossary"}, ref="v1.2.3")

    def test_a_link_to_another_page_of_the_manual_stays_on_the_site(self):
        self.assertEqual(self.link("cli.md"), "../cli/")
        self.assertEqual(self.link("glossary.md#epic"), "../glossary/#epic")
        self.assertEqual(self.link("cli.md", here="docs/"), "cli/")
        self.assertEqual(self.link("#plant-the-tracker"), "#plant-the-tracker")

    def test_anything_else_in_the_repository_goes_to_github_at_the_release(self):
        self.assertEqual(self.link("lessons/grep-exit-code.md"), f"{gen.REPO_URL}/blob/v1.2.3/docs/lessons/grep-exit-code.md")
        self.assertEqual(self.link("../CONTRIBUTING.md#releasing"), f"{gen.REPO_URL}/blob/v1.2.3/CONTRIBUTING.md#releasing")
        self.assertEqual(self.link("../scripts/"), f"{gen.REPO_URL}/tree/v1.2.3/scripts")
        self.assertEqual(self.link("https://example.com/a.md"), "https://example.com/a.md")

    def test_a_root_relative_link_is_read_from_the_repository_root(self):
        self.assertEqual(self.link("/docs/cli.md#x"), "../cli/#x")
        self.assertEqual(self.link("/CONTRIBUTING.md"), f"{gen.REPO_URL}/blob/v1.2.3/CONTRIBUTING.md")

    def test_a_tag_name_is_quoted_into_the_url(self):
        url = gen.resolve_doc_link("../README.md", here="docs/x/", slugs=set(), ref='v1"><b>')
        self.assertEqual(url, f"{gen.REPO_URL}/blob/v1%22%3E%3Cb%3E/README.md")

    def test_an_english_only_page_leaves_the_home_redirect_alone(self):
        # a Korean visitor reaches /docs/ from /ko/; the logo must not pin them to the English home
        pages = gen.build_release_pages(gen.REPO / "docs", None, "v1.2.3")
        page = pages[Path("docs/workflow/index.html")]
        self.assertIn('class="brand" href="../../"', page)
        self.assertIn('href="../../#see"', page)
        self.assertNotIn("?lang=en", page)

    def test_the_manual_leaves_lessons_out_and_keeps_its_order(self):
        import tempfile

        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            (root / "lessons").mkdir()
            (root / "lessons" / "x.md").write_text("# Lesson\n\nNo.\n")
            (root / "zeta.md").write_text("# Zeta\n\nLast, by name.\n")
            (root / "workflow.md").write_text("# Day-to-day\n\nHow work *moves* in `x86_64` [land](cli.md).\n\n## A\n")
            pages = gen.doc_pages(root)
        self.assertEqual([p[0] for p in pages], ["workflow", "zeta"])
        self.assertEqual(pages[0][1], "Day-to-day")
        self.assertEqual(pages[0][2], "How work moves in x86_64 land.")

    def test_a_lede_reads_the_way_the_page_draws_it(self):
        import tempfile

        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            # a code span keeps the link it shows; a fence or a list before the first paragraph is not the lede
            (root / "a.md").write_text("# A\n\nLink one — `[epic](glossary.md#epic)` &amp; \\*.\n")
            (root / "b.md").write_text("```sh\n# not the title\n```\n\n# B\n\n```sh\nmoai status\n```\n\n- item\n  more\n\nThe lede.\n")
            pages = {p[0]: p for p in gen.doc_pages(root)}
        self.assertEqual(pages["a"][2], "Link one — [epic](glossary.md#epic) & *.")
        self.assertEqual(pages["b"][1], "B")
        self.assertEqual(pages["b"][2], "The lede.")

    def test_the_manual_builds_from_this_checkout(self):
        pages = gen.build_release_pages(gen.REPO / "docs", None, "v1.2.3")
        self.assertIn(Path("docs/index.html"), pages)
        self.assertIn(Path("docs/workflow/index.html"), pages)
        self.assertFalse(any("lessons" in str(p) for p in pages))
        page = pages[Path("docs/workflow/index.html")]
        self.assertIn('<html lang="en">', page)
        self.assertIn(f"{gen.REPO_URL}/blob/v1.2.3/docs/workflow.md", page)
        self.assertNotIn("{{", page)


class Changelog(unittest.TestCase):
    SRC = """# Changelog

How versions are picked — a note to contributors.

## [Unreleased]

### Added

- not shipped yet

## [0.2.0] - 2026-10-02

**One thing breaks a caller.**

### Added

- a page of the [manual](glossary.md#epic) and [the guide](docs/workflow.md)

## [0.1.0] - 2026-09-01

### Added

- the first, with <id> as text
"""

    def test_released_versions_only_newest_first(self):
        self.assertEqual([v for v, _d, _b in gen.changelog_sections(self.SRC)], ["0.2.0", "0.1.0"])

    def test_each_release_keeps_its_own_anchors(self):
        toc, html = gen.changelog_page(self.SRC, slugs={"glossary", "workflow"}, ref="v0.2.0")
        self.assertIn('href="#v0.2.0"', toc)
        self.assertIn('id="v0.2.0-added"', html)
        self.assertIn('id="v0.1.0-added"', html)
        self.assertNotIn("not shipped yet", html)
        self.assertNotIn("note to contributors", html)
        self.assertIn('href="../docs/glossary/#epic"', html)
        self.assertIn('href="../docs/workflow/"', html)
        self.assertIn("&lt;id&gt;", html)
        self.assertIn('<span class="release-date">2026-10-02</span>', html)

    def test_a_heading_of_a_release_s_own_stays_in_it(self):
        src = "## [0.2.0]\n\n### Added\n\n- a\n\n## Upgrade notes\n\n- important\n\nA thing\n---\n\n## [0.1.0]\n\nA thing\n---\n"
        self.assertEqual([v for v, _d, _b in gen.changelog_sections(src)], ["0.2.0", "0.1.0"])
        _toc, html = gen.changelog_page(src, slugs=set(), ref="v0.2.0")
        self.assertIn("important", html)
        ids = re.findall(r'id="([^"]+)"', html)
        self.assertEqual(len(ids), len(set(ids)), ids)
        self.assertIn("v0.2.0-upgrade-notes", ids)

    def test_a_changelog_with_no_release_stops_the_build(self):
        with self.assertRaises(gen.BuildError):
            gen.changelog_page("# Changelog\n\n## [Unreleased]\n", slugs=set(), ref="v0")


class Site(unittest.TestCase):
    def media_names(self):
        return {
            name for template in gen.HERE.glob("*.html")
            for name in re.findall(r"assets/media/([\w.-]+)", template.read_text(encoding="utf-8"))
        }

    def committed_media(self):
        return {p.name for p in (gen.SITE / "assets/media").iterdir() if p.is_file()}

    def test_every_media_file_the_pages_name_is_committed(self):
        self.assertEqual(self.media_names() - self.committed_media(), set())

    def test_no_committed_media_file_is_left_unnamed(self):
        self.assertEqual(self.committed_media() - self.media_names(), set())

    def test_record_site_builds_every_media_but_the_team_scene(self):
        script = (gen.REPO / "scripts/site-demo/record-site.sh").read_text(encoding="utf-8")
        media = re.search(r"\bMEDIA=\((.*?)\)", script, re.S)
        self.assertIsNotNone(media)
        self.assertEqual(set(media.group(1).split()), self.media_names() - {"tui-team.cast"})

    def test_the_committed_pages_are_what_the_generator_builds(self):
        # CI runs `gen.py --check` too; here it fails with the page named.
        for rel_path, text in gen.build_pages().items():
            on_disk = (gen.SITE / rel_path).read_text(encoding="utf-8")
            self.assertEqual(on_disk, text, f"site/{rel_path} is stale — run python3 scripts/site/gen.py")


if __name__ == "__main__":
    unittest.main()
