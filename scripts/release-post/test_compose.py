"""compose.py 의 시험. 저장소의 Rust 시험과 따로 돈다 — `python3 -I -m unittest discover scripts/release-post`.

discord.yml 이 올리기 전에 이것을 먼저 돌린다. 글을 짓는 규칙이 깨진 채로 공개 채널에 나가지 않게.
"""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import compose  # noqa: E402

CHANGELOG = """# Changelog

Intro text that is not a release.

## [Unreleased]

### Fixed

- Something not released yet.

## [0.23.0] - 2026-10-23

### Added

- **Your own keys for the explorer.** A `keys.toml` next to your user config
  (or wherever `MOAI_KEYS` points) changes the keys of `moai tui` line by line.
  Other commands never open the file. (moai-pxiy)
- **`moai keys`** shows the keys the explorer runs on, marks the ones your file
  changed and lists the lines it dropped and why (on stderr, exit 0).
  `--template` prints a starter file. (moai-so7w)

### Fixed

- The board no longer wraps at 80 columns. (moai-ab12, moai-cd34.x9z)

### Security

- **The installer checks the archive.** Nothing installs without `SHA256SUMS`.

## [0.2.0] - 2026-09-01

### Added

- Old stuff.
"""


class Section(unittest.TestCase):
    def test_takes_only_the_asked_release(self):
        body = compose.section(CHANGELOG, "0.23.0")
        self.assertIn("Your own keys", body)
        self.assertNotIn("Old stuff", body)
        self.assertNotIn("not released yet", body)

    def test_a_leading_v_is_the_same_release(self):
        self.assertEqual(compose.section(CHANGELOG, "v0.23.0"), compose.section(CHANGELOG, "0.23.0"))

    def test_a_dot_is_not_a_wildcard(self):
        # release.yml 이 머리줄을 정규식으로 안 짜는 까닭과 같다 — `0.23.0` 이 `0x23y0` 을 집으면 안 된다
        self.assertIsNone(compose.section(CHANGELOG.replace("[0.23.0]", "[0x23y0]"), "0.23.0"))

    def test_a_missing_release_is_none(self):
        self.assertIsNone(compose.section(CHANGELOG, "9.9.9"))


class Parse(unittest.TestCase):
    def test_items_fold_their_continuation_lines(self):
        parts = compose.parse(compose.section(CHANGELOG, "0.23.0"))
        self.assertEqual(len(parts["Added"]), 2)
        self.assertTrue(parts["Added"][0].startswith("**Your own keys for the explorer.** A `keys.toml`"))
        self.assertIn("line by line. Other commands", parts["Added"][0])
        self.assertEqual(len(parts["Fixed"]), 1)
        self.assertEqual(len(parts["Security"]), 1)


class Summarize(unittest.TestCase):
    def test_a_bold_headline_stands_alone(self):
        item = "**Your own keys for the explorer.** A `keys.toml` changes the keys. (moai-pxiy)"
        self.assertEqual(compose.summarize(item), "**Your own keys for the explorer**")

    def test_a_bold_name_keeps_its_first_sentence(self):
        item = "**`moai keys`** shows the keys the explorer runs on. `--template` prints a starter file. (moai-so7w)"
        self.assertEqual(compose.summarize(item), "**`moai keys`** shows the keys the explorer runs on")

    def test_tracker_ids_are_dropped(self):
        self.assertEqual(compose.summarize("The board no longer wraps at 80 columns. (moai-ab12, moai-cd34.x9z)"),
                         "The board no longer wraps at 80 columns")

    def test_a_period_inside_code_does_not_end_the_sentence(self):
        item = "Reads `.moai/config.toml` once per run. Then it does more."
        self.assertEqual(compose.summarize(item), "Reads `.moai/config.toml` once per run")

    def test_an_aside_after_a_colon_or_dash_is_dropped(self):
        item = "**`moai web`** serves the tracker as a page that never writes it: a board, a list and the wiki."
        self.assertEqual(compose.summarize(item), "**`moai web`** serves the tracker as a page that never writes it")
        item = "The installer upgrades in place — it reads the old version first and says where it went."
        self.assertEqual(compose.summarize(item), "The installer upgrades in place")
        item = "It opens the detail of a sibling's row; it used to say the row was not there."
        self.assertEqual(compose.summarize(item), "It opens the detail of a sibling's row")

    def test_a_colon_inside_code_is_not_an_aside(self):
        item = "Serves on `127.0.0.1:6624` by default."
        self.assertEqual(compose.summarize(item), "Serves on `127.0.0.1:6624` by default")

    def test_a_trailing_parenthesis_is_dropped(self):
        item = "**`moai keys`** lists the lines it dropped and why (on stderr, exit 0). More."
        self.assertEqual(compose.summarize(item), "**`moai keys`** lists the lines it dropped and why")

    def test_a_long_sentence_is_cut_at_a_word(self):
        item = "word " * 80
        out = compose.summarize(item)
        self.assertLessEqual(len(out), compose.ITEM_CHARS)
        self.assertTrue(out.endswith("…"))

    def test_a_cut_never_leaves_a_marker_open(self):
        # 0.7.0·0.11.0·0.17.0 의 굵은 머리말이 160자를 넘어, 닫는 ** 가 잘린 채 나갔다
        item = "**`moai tui --json` puts `progress` on every folder row " + "and more words " * 12 + "here.** Rest."
        out = compose.summarize(item)
        self.assertLessEqual(len(out), compose.ITEM_CHARS)
        self.assertTrue(out.endswith("…**"), out)
        out = compose.summarize("Some `code with a long span " + "y" * 200 + "` end.")
        self.assertEqual(out.count("`") % 2, 0, out)
        self.assertLessEqual(len(out), compose.ITEM_CHARS)

    def test_an_aside_cut_inside_bold_closes_it(self):
        item = "**A long bold headline about something important: with colon inside** then text."
        self.assertEqual(compose.summarize(item), "**A long bold headline about something important**")

    def test_a_hyphenated_word_in_parentheses_is_not_an_id(self):
        self.assertEqual(compose.summarize("Adds a (read-only) page. More."), "Adds a (read-only) page")

    def test_an_abbreviation_does_not_end_the_sentence(self):
        self.assertEqual(compose.summarize("Use a filter, e.g. `-s todo`, to narrow it. More."),
                         "Use a filter, e.g. `-s todo`, to narrow it")


class Render(unittest.TestCase):
    def test_big_changes_come_before_fixes(self):
        post = compose.compose(CHANGELOG, "v0.23.0", "buzzni/moai")
        self.assertTrue(post.startswith("## moai v0.23.0"))
        self.assertLess(post.index("Your own keys"), post.index("80 columns"))
        self.assertLess(post.index("80 columns"), post.index("installer checks"))
        self.assertIn("<https://github.com/buzzni/moai/releases/tag/v0.23.0>", post)
        self.assertNotIn("moai-pxiy", post)

    def test_an_empty_release_still_announces_itself(self):
        post = compose.compose("## [1.0.0] - 2026-11-01\n", "1.0.0", "buzzni/moai")
        self.assertIn("moai v1.0.0", post)
        self.assertIn("releases/tag/v1.0.0", post)

    def test_a_missing_release_is_an_error(self):
        with self.assertRaises(compose.NoRelease):
            compose.compose(CHANGELOG, "9.9.9", "buzzni/moai")

    def test_each_group_shows_a_handful(self):
        many = "\n".join(f"- Change {i}." for i in range(10))
        post = compose.compose(f"## [2.1.0]\n\n### Added\n\n{many}\n", "2.1.0", "buzzni/moai")
        self.assertEqual(post.count("\n- Change "), compose.MAX_ITEMS)
        self.assertIn(f"…and {10 - compose.MAX_ITEMS} more in the release notes", post)

    def test_the_post_fits_one_discord_message(self):
        many = "\n".join(f"- Change number {i} does something rather long to describe here." for i in range(200))
        log = f"## [2.0.0] - 2026-12-01\n\n### Added\n\n{many}\n\n### Fixed\n\n{many}\n"
        post = compose.compose(log, "2.0.0", "buzzni/moai")
        self.assertLessEqual(len(post), compose.DISCORD_LIMIT)
        self.assertIn("more in the release notes", post)
        # 줄인 판에도 두 갈래가 다 선다 — 큰 변경만 남기고 버그를 통째로 버리지 않는다
        self.assertIn("### Fixes", post)


if __name__ == "__main__":
    unittest.main()
