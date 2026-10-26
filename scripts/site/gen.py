#!/usr/bin/env python3
"""Build the moai site (site/) from one template per page and one text bundle per language.

    python3 scripts/site/gen.py            write site/index.html, site/{ko,ja}/…, site/guide/…
    python3 scripts/site/gen.py --check    write nothing; exit 1 and name each page that is stale

The pages under site/ that this writes are committed, and CI runs `--check` — the same shape as
web/'s committed bundle. Edit the templates (layout.html, home.html, guide.html) and the bundles
(text/<lang>.toml) here, never the generated pages: the next run overwrites them.

The structure lives once, in the templates; the words live per language, in the bundles. `{{a.b}}`
in a template is the key `b` of table `[a]` in the page's bundle. A missing key stops the build —
an empty slot on one language's page is worse than no build.

Standard library only (Python 3.11+, for tomllib).
"""

from __future__ import annotations

import argparse
import posixpath
import re
import sys
import tomllib
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent
SITE = REPO / "site"
BASE_URL = "https://moai.buzzni.com/"

LANGS = ("en", "ko", "ja")
LANG_NAMES = {"en": "English", "ko": "한국어", "ja": "日本語"}

# page name -> its directory under site/, per language ("" is the site root)
PAGES = {
    "home": {"en": "", "ko": "ko/", "ja": "ja/"},
    "guide": {"en": "guide/", "ko": "ko/guide/", "ja": "ja/guide/"},
}
OG_TYPE = {"home": "website", "guide": "article"}

# The English home is the root, and it sends a visitor on to /ko/ or /ja/ before it draws — by the
# language picked by hand before (site.js keeps it), else by the browser's. Only that page carries it.
REDIRECT = """  <script>
    (function () {
      // ?lang=en 은 다른 언어 페이지의 English 링크가 붙인다 — 저장소가 막힌 브라우저에서도 영어에 머물게
      if (/(^|[?&])lang=en(&|$)/.test(location.search)) return;
      var pick = null;
      try { pick = localStorage.getItem('moai-lang'); } catch (e) {}
      if (!pick) {
        var langs = navigator.languages || [navigator.language || ''];
        for (var i = 0; i < langs.length && !pick; i++) {
          var l = String(langs[i]).toLowerCase();
          if (l.indexOf('ko') === 0) pick = 'ko';
          else if (l.indexOf('ja') === 0) pick = 'ja';
          else if (l.indexOf('en') === 0) pick = 'en';
        }
      }
      if (pick === 'ko' || pick === 'ja') location.replace(pick + '/' + location.search + location.hash);
    })();
  </script>"""

SPINNER = (
    '<span class="spin" aria-hidden="true"><svg viewBox="0 0 10 15">'
    + "".join(
        f'<circle class="d{n}" cx="{x}" cy="{y}" r="1.8"/>'
        for n, x, y in [(1, 2.6, 2.6), (2, 2.6, 7.5), (3, 2.6, 12.4), (4, 7.4, 2.6), (5, 7.4, 7.5), (6, 7.4, 12.4)]
    )
    + "</svg></span>"
)

SLOT = re.compile(r"\{\{([\w.-]+)\}\}")
_WIDE = re.compile(r"[ᄀ-ᅟ⺀-〾ぁ-㏿㐀-䶿一-鿿가-힣豈-﫿︰-﹏＀-｠￠-￦]")


class BuildError(Exception):
    pass


def load_bundle(lang: str) -> dict:
    with open(HERE / "text" / f"{lang}.toml", "rb") as f:
        return tomllib.load(f)


def lookup(bundle: dict, key: str, lang: str) -> str:
    node = bundle
    for part in key.split("."):
        if not isinstance(node, dict) or part not in node:
            raise BuildError(f"text/{lang}.toml has no `{key}`")
        node = node[part]
    if not isinstance(node, str):
        raise BuildError(f"text/{lang}.toml: `{key}` is not a string")
    return node


def fill(template: str, values: dict[str, str], bundle: dict, lang: str) -> str:
    """Put each `{{slot}}` in: a computed value first, else the bundle's text. A line that holds only a
    slot whose value is empty goes away whole, so an optional block leaves no blank line behind."""

    def one(m: re.Match) -> str:
        key = m.group(1)
        return values[key] if key in values else lookup(bundle, key, lang)

    out = []
    for line in template.split("\n"):
        m = SLOT.fullmatch(line.strip())
        if m and m.group(1) in values and values[m.group(1)] == "":
            continue
        out.append(SLOT.sub(one, line))
    return "\n".join(out)


def rel(from_dir: str, to_dir: str) -> str:
    """The href from a page in `from_dir` to the page in `to_dir`, both relative to site/."""
    r = posixpath.relpath(to_dir or ".", from_dir or ".")
    return "./" if r == "." else r + "/"


def page_href(from_dir: str, to_dir: str) -> str:
    # A link into the English home from any other page carries ?lang=en, so the home's redirect does
    # not send someone who chose English back to the language their browser asks for.
    href = rel(from_dir, to_dir)
    return href + "?lang=en" if to_dir == "" and from_dir != "" else href


def glint_ms(headline: str) -> int:
    # moai web's glintTime: (cells + band 3 + rest 8) × 120ms. A wide character is two cells
    plain = re.sub(r"<[^>]+>", "", headline)
    return (sum(2 if _WIDE.match(c) else 1 for c in plain) + 3 + 8) * 120


def rotator(bundle: dict, lang: str) -> str:
    rot = bundle.get("home", {}).get("rotator")
    if not rot or not rot.get("slides"):
        raise BuildError(f"text/{lang}.toml has no [[home.rotator.slides]]")
    label = rot["label"]
    n = len(rot["slides"])
    slides, tabs = [], []
    for i, s in enumerate(rot["slides"]):
        on = i == 0
        head, ms = s["headline"], glint_ms(s["headline"])
        slides.append(
            f'        <div class="slide{" is-on" if on else ""}" id="slide-{i + 1}" role="tabpanel" aria-roledescription="slide" '
            f'aria-label="{i + 1} / {n}" aria-labelledby="tab-{i + 1}"{"" if on else " hidden"} style="--glint-time: {ms}ms">\n'
            f'          <p class="headline"><span class="held-body"><span class="held-text">{head}</span>'
            f'<span class="held-glint" aria-hidden="true"><span>{head}</span></span></span></p>\n'
            f'          <p class="lede">{s["lede"]}</p>\n'
            f"        </div>\n"
        )
        tabs.append(
            f'      <button class="rot-tab" id="tab-{i + 1}" role="tab" aria-controls="slide-{i + 1}" aria-selected="{"true" if on else "false"}"'
            f'{"" if on else " tabindex=\"-1\""} style="--rot-time: {max(8000, ms + 3000)}ms"><span class="rot-n">{i + 1:02d}</span>'
            f'<span class="rot-name">{s["tab"]}</span><i class="rot-bar" aria-hidden="true"></i></button>\n'
        )
    return (
        f'    <section class="rotator" aria-roledescription="carousel" aria-label="{label}">\n'
        f'      <div class="rot-stage">\n        {SPINNER}\n      <div class="rot-slides">\n'
        + "".join(slides)
        + "      </div>\n      </div>\n"
        + f'      <div class="rot-tabs" role="tablist" aria-label="{label}">\n'
        + "".join(tabs)
        + "      </div>\n    </section>"
    )


def chrome(page: str, lang: str, here: str, bundle: dict, *, langs: dict[str, str]) -> dict[str, str]:
    """The computed slots of layout.html for one page: paths, the language picker, nav and footer.
    `langs` maps each language to the directory of this page in it; a page that stands in English only
    sends the picker to the other languages' homes."""
    root = rel(here, "")
    if root == "./":
        root = ""
    home_dir = PAGES["home"][lang]
    guide_dir = PAGES["guide"][lang]

    def link(to_dir: str, label: str, *, anchor: str = "", mark: bool = True) -> str:
        # the nav marks the page you are on; the footer only links to it
        if to_dir == here and not anchor:
            return f'      <a href="./"{" aria-current=\"page\"" if mark else ""}>{label}</a>'
        href = anchor if to_dir == here else page_href(here, to_dir) + anchor
        return f'      <a href="{href}">{label}</a>'

    nav = "\n".join(
        [
            link(home_dir, lookup(bundle, "nav.see", lang), anchor="#see"),
            link(guide_dir, lookup(bundle, "nav.guide", lang)),
            f'      <a href="https://github.com/buzzni/moai/tree/main/docs" hidden>{lookup(bundle, "nav.docs", lang)}</a>'
            "<!-- 문서 페이지가 서면 hidden 을 걷는다: moai-qrxu -->",
            f'      <a href="https://github.com/buzzni/moai/blob/main/CHANGELOG.md" hidden>{lookup(bundle, "nav.changelog", lang)}</a>'
            "<!-- 변경 기록 페이지가 서면 hidden 을 걷는다: moai-gw11 -->",
        ]
    )
    footer = "\n".join(
        [
            f'      <a href="https://github.com/buzzni/moai/tree/main/docs" hidden>{lookup(bundle, "nav.docs", lang)}</a>'
            "<!-- 문서 페이지가 서면 hidden 을 걷는다: moai-qrxu -->",
            f'      <a href="https://github.com/buzzni/moai/blob/main/CHANGELOG.md" hidden>{lookup(bundle, "nav.changelog", lang)}</a>'
            "<!-- 변경 기록 페이지가 서면 hidden 을 걷는다: moai-gw11 -->",
            link(guide_dir, lookup(bundle, "nav.guide", lang), mark=False),
        ]
    )
    switch = []
    for code in LANGS:
        to = langs.get(code, PAGES["home"][code])
        cur = code == lang
        href = "./" if cur else page_href(here, to)
        switch.append(
            f'<a href="{href}" hreflang="{code}" lang="{code}" data-lang="{code}"{" aria-current=\"page\"" if cur else ""}>'
            f"{LANG_NAMES[code]}</a>"
        )
    alternates = ""
    if len(langs) > 1:
        alternates = "\n".join(
            [f'  <link rel="alternate" hreflang="{c}" href="{BASE_URL}{d}">' for c, d in langs.items()]
            + [f'  <link rel="alternate" hreflang="x-default" href="{BASE_URL}{langs["en"]}">']
        )
    return {
        "lang": lang,
        "root": root,
        "url": BASE_URL + here,
        "og_type": OG_TYPE.get(page, "article"),
        "alternates": alternates,
        "redirect": REDIRECT if page == "home" and lang == "en" else "",
        "home_href": rel(here, home_dir) if here == home_dir else page_href(here, home_dir),
        "nav_links": nav,
        "footer_links": footer,
        "lang_switch": '<span aria-hidden="true">|</span>'.join(switch),
    }


def build_pages() -> dict[Path, str]:
    """Every generated page of the committed site: path under site/ -> its text."""
    layout = (HERE / "layout.html").read_text(encoding="utf-8")
    out: dict[Path, str] = {}
    for lang in LANGS:
        bundle = load_bundle(lang)
        for page, dirs in PAGES.items():
            here = dirs[lang]
            values = chrome(page, lang, here, bundle, langs=dirs)
            values["title"] = lookup(bundle, f"{page}.title", lang)
            values["description"] = lookup(bundle, f"{page}.description", lang)
            values["styles"] = (
                f'  <link rel="stylesheet" href="{values["root"]}assets/vendor/asciinema-player.css">\n'
                f'  <link rel="stylesheet" href="{values["root"]}assets/site.css">'
            )
            values["scripts"] = (
                f'<script src="{values["root"]}assets/vendor/asciinema-player.min.js"></script>\n'
                f'<script src="{values["root"]}assets/site.js"></script>'
            )
            body_values = {"root": values["root"]}
            if page == "home":
                body_values["rotator"] = rotator(bundle, lang)
            body = fill((HERE / f"{page}.html").read_text(encoding="utf-8").strip("\n"), body_values, bundle, lang)
            values["main"] = body
            out[Path(here) / "index.html"] = fill(layout, values, bundle, lang)
    return out


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--check", action="store_true", help="write nothing; exit 1 if a committed page is stale")
    args = ap.parse_args(argv)
    try:
        pages = build_pages()
    except BuildError as e:
        print(f"gen.py: {e}", file=sys.stderr)
        return 2
    stale = []
    for rel_path, text in sorted(pages.items()):
        path = SITE / rel_path
        old = path.read_text(encoding="utf-8") if path.exists() else None
        if old == text:
            continue
        stale.append(rel_path)
        if not args.check:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text, encoding="utf-8")
    if args.check:
        for p in stale:
            print(f"site/{p} is stale — run python3 scripts/site/gen.py and commit it", file=sys.stderr)
        return 1 if stale else 0
    for p in stale:
        print(f"wrote site/{p}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
