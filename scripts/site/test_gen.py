"""Tests for gen.py — run with `python3 -I -B -m unittest discover -s scripts/site`."""

from __future__ import annotations

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


class Site(unittest.TestCase):
    def test_the_committed_pages_are_what_the_generator_builds(self):
        # CI runs `gen.py --check` too; here it fails with the page named.
        for rel_path, text in gen.build_pages().items():
            on_disk = (gen.SITE / rel_path).read_text(encoding="utf-8")
            self.assertEqual(on_disk, text, f"site/{rel_path} is stale — run python3 scripts/site/gen.py")


if __name__ == "__main__":
    unittest.main()
