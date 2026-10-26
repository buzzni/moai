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
from html import escape
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))  # `python3 -I` leaves the script's own directory off the path

import mdhtml  # noqa: E402

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

# The manual (docs/*.md) and the changelog are built from the newest release tag by the Pages workflow
# and are not committed (2026-10-26, the person's decision on moai-s7f7): they change with every epic,
# and a committed copy would have every epic regenerate the site or turn CI red. They stand in English
# only — the wiki is written in English — and the Korean and Japanese menus point at them.
DOCS_DIR = "docs/"
CHANGELOG_DIR = "changelog/"
REPO_URL = "https://github.com/buzzni/moai"
# the order the manual's pages stand in; a page not named here follows, by name
# a page whose first paragraph is not about the page (cli.md opens with a note to contributors)
DOC_LEDE = {"cli": "Every moai command and its flags, as moai --help prints them."}
DOC_ORDER = ["workflow", "explorer", "web", "agents", "cli", "json", "recovery", "windows", "glossary"]

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

    def link(to_dir: str, label: str, *, anchor: str = "", mark: bool = True, english: bool = False) -> str:
        # the nav marks the page you are on; the footer only links to it. A page that stands in English
        # only says so to the browser, so a Korean or Japanese menu can point at it honestly
        lang_attr = ' hreflang="en"' if english and lang != "en" else ""
        if to_dir == here and not anchor:
            return f'      <a href="./"{lang_attr}{" aria-current=\"page\"" if mark else ""}>{label}</a>'
        href = anchor if to_dir == here else page_href(here, to_dir) + anchor
        return f'      <a href="{href}"{lang_attr}>{label}</a>'

    docs = link(DOCS_DIR, lookup(bundle, "nav.docs", lang), english=True)
    changelog = link(CHANGELOG_DIR, lookup(bundle, "nav.changelog", lang), english=True)
    nav = "\n".join(
        [
            link(home_dir, lookup(bundle, "nav.see", lang), anchor="#see"),
            link(guide_dir, lookup(bundle, "nav.guide", lang)),
            docs,
            changelog,
        ]
    )
    footer = "\n".join(
        [
            link(DOCS_DIR, lookup(bundle, "nav.docs", lang), mark=False, english=True),
            link(CHANGELOG_DIR, lookup(bundle, "nav.changelog", lang), mark=False, english=True),
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


def _plain(md: str) -> str:
    """A markdown line's text as a person reads it: links to their words, code and emphasis bare."""
    md = re.sub(r"!?\[([^\]]*)\]\([^)]*\)", r"\1", md)
    md = re.sub(r"\*+|`+|(?<!\w)_+|_+(?!\w)", "", md)  # `x86_64` keeps its underscore
    return md.strip()


def doc_pages(docs_dir: Path) -> list[tuple[str, str, str, str]]:
    """The manual's top-level pages as (slug, title, lede, source), in DOC_ORDER. docs/lessons/ and
    any other subdirectory stay out — they are this repository's notes to its agents, not the manual."""
    found = {}
    for path in sorted(docs_dir.glob("*.md")):
        src = path.read_text(encoding="utf-8")
        lines = src.split("\n")
        title = next((_plain(l[2:]) for l in lines if l.startswith("# ")), path.stem)
        para, seen_h1 = [], False
        for line in lines:
            if line.startswith("# "):
                seen_h1 = True
                continue
            if not seen_h1:
                continue
            if not line.strip():
                if para:
                    break
                continue
            if line.startswith(("#", "    ", "```", "|", "- ", "* ")):
                if para:
                    break
                continue
            para.append(line.strip())
        lede = DOC_LEDE.get(path.stem) or _plain(" ".join(para))
        found[path.stem] = (path.stem, title, lede, src)
    order = [s for s in DOC_ORDER if s in found] + sorted(s for s in found if s not in DOC_ORDER)
    return [found[s] for s in order]


def resolve_doc_link(href: str, *, here: str, slugs: set[str], ref: str, base: str = "docs") -> str:
    """Where a link in a wiki page (or, with base "", in CHANGELOG.md) goes on the site. A link to a page
    of the manual stays on the site; a link to anything else in the repository goes to it on GitHub, at
    the release it was read from."""
    if re.match(r"^[a-z][a-z0-9+.-]*:", href, re.I) or href.startswith("#"):
        return href
    path, _, frag = href.partition("#")
    frag = "#" + frag if frag else ""
    norm = posixpath.normpath(posixpath.join(base, path)) if path else ""
    if base == "" and "/" not in norm and norm.endswith(".md") and norm[:-3] in slugs:
        norm = "docs/" + norm  # the changelog names a wiki page by its file name alone
    if norm.startswith("docs/") and norm.endswith(".md") and norm[5:-3] in slugs:
        return rel(here, f"{DOCS_DIR}{norm[5:-3]}/") + frag
    if norm.startswith("../") or norm == "..":
        return href  # outside the repository: leave it as written
    kind = "tree" if path.endswith("/") or "." not in posixpath.basename(norm) else "blob"
    return f"{REPO_URL}/{kind}/{ref}/{norm}{frag}"


def doc_toc(pages, here: str, current: str | None, headings) -> str:
    items = []
    for slug, title, _lede, _src in pages:
        to = f"{DOCS_DIR}{slug}/"
        if slug == current:
            sub = "".join(
                f'\n          <li><a href="#{h.anchor}">{escape(h.text)}</a></li>' for h in headings if h.level == 2
            )
            nested = f"\n        <ol>{sub}\n        </ol>\n      " if sub else ""
            items.append(f'      <li><a href="./" aria-current="page">{escape(title)}</a>{nested}</li>')
        else:
            items.append(f'      <li><a href="{rel(here, to)}">{escape(title)}</a></li>')
    return "    <ol>\n" + "\n".join(items) + "\n    </ol>"


def release_label(ref: str) -> str:
    return f"moai {ref}" if re.fullmatch(r"v\d+\.\d+\.\d+.*", ref) else f"moai at {ref}"


def build_release_pages(docs_dir: Path | None, changelog: Path | None, ref: str) -> dict[Path, str]:
    """The manual and the changelog, read from a release (the Pages workflow hands in the newest v* tag's
    files). English only. Not committed — `.gitignore` keeps them out of git."""
    layout = (HERE / "layout.html").read_text(encoding="utf-8")
    doc_tmpl = (HERE / "doc.html").read_text(encoding="utf-8").strip("\n")
    lang, bundle = "en", load_bundle("en")
    out: dict[Path, str] = {}

    def page(here: str, title: str, description: str, body: dict[str, str]) -> None:
        values = chrome("doc", lang, here, bundle, langs={"en": here})
        values.update(
            title=escape(title),
            description=escape(description, quote=True),
            styles=f'  <link rel="stylesheet" href="{values["root"]}assets/site.css">',
            scripts=f'<script src="{values["root"]}assets/site.js"></script>',
            main=fill(doc_tmpl, body, bundle, lang),
        )
        out[Path(here) / "index.html"] = fill(layout, values, bundle, lang)

    pages = doc_pages(docs_dir) if docs_dir is not None else []
    slugs = {p[0] for p in pages}
    if changelog is not None:
        toc, content = changelog_page(changelog.read_text(encoding="utf-8"), slugs=slugs, ref=ref)
        page(
            CHANGELOG_DIR,
            "moai changelog",
            f"What changed in each release of moai, up to {release_label(ref)}.",
            {
                "toc": toc,
                "crumb": f'{escape(release_label(ref))} · <a href="{REPO_URL}/blob/{ref}/CHANGELOG.md">source</a>',
                "content": content,
            },
        )
    if docs_dir is not None:
        source = f'<a href="{REPO_URL}/tree/{ref}/docs">source</a>'
        index = "\n".join(
            [
                "    <h1>moai documentation</h1>",
                f"    <p>The manual of {escape(release_label(ref))}: the same pages <code>moai wiki</code> reads "
                "in the repository. Start with the day-to-day workflow; the CLI reference lists every command and flag.</p>",
                '    <ul class="doc-index">',
                *[
                    f'      <li><a href="{slug}/">{escape(title)}</a><span>{escape(lede)}</span></li>'
                    for slug, title, lede, _src in pages
                ],
                "    </ul>",
            ]
        )
        page(
            DOCS_DIR,
            "moai documentation",
            f"The manual of {release_label(ref)}: workflow, the explorer, moai web, agents, the CLI and recovery.",
            {"toc": doc_toc(pages, DOCS_DIR, None, []), "crumb": f"{escape(release_label(ref))} · {source}", "content": index},
        )
        for slug, title, lede, src in pages:
            here = f"{DOCS_DIR}{slug}/"
            html, headings = mdhtml.render(
                src, lambda href, here=here: resolve_doc_link(href, here=here, slugs=slugs, ref=ref)
            )
            crumb = (
                f'<a href="../">Docs</a> · {escape(release_label(ref))} · '
                f'<a href="{REPO_URL}/blob/{ref}/docs/{slug}.md">source</a>'
            )
            page(
                here,
                f"{title} — moai docs",
                lede or title,
                {"toc": doc_toc(pages, here, slug, headings), "crumb": crumb, "content": html},
            )
    return out


RELEASE_HEAD = re.compile(r"^\[([^\]]+)\](?:\s+-\s+(\d{4}-\d{2}-\d{2}))?\s*$")


def changelog_sections(src: str) -> list[tuple[str, str, str]]:
    """CHANGELOG.md's releases as (version, date, body), newest first. The text above the first release
    (the note to contributors on how versions are picked) and [Unreleased] stay out — a reader of the
    site wants what shipped."""
    out = []
    for chunk in re.split(r"^## ", src, flags=re.M)[1:]:
        head, _, body = chunk.partition("\n")
        m = RELEASE_HEAD.match(head.strip())
        if not m or m.group(1).lower() == "unreleased":
            continue
        out.append((m.group(1), m.group(2) or "", body.strip("\n")))
    return out


def changelog_page(src: str, *, slugs: set[str], ref: str) -> tuple[str, str]:
    """(toc, content) of the changelog page. Each release is rendered on its own, so its `### Added`
    takes the release's id in front of its own — the anchors stay unique down the page."""
    here = CHANGELOG_DIR
    toc, parts = [], []
    for version, date, body in changelog_sections(src):
        vid = "v" + re.sub(r"[^\w.-]", "-", version)
        html, _ = mdhtml.render(
            body, lambda href: resolve_doc_link(href, here=here, slugs=slugs, ref=ref, base="")
        )
        html = re.sub(r'<(h[3-6]) id="([^"]+)">', lambda m: f'<{m.group(1)} id="{vid}-{m.group(2)}">', html)
        when = f' <span class="release-date">{escape(date)}</span>' if date else ""
        parts.append(
            f'    <section class="release" aria-labelledby="{vid}">\n'
            f'    <h2 id="{vid}"><a href="#{vid}">{escape(version)}</a>{when}</h2>\n{html}\n    </section>'
        )
        toc.append(f'      <li><a href="#{vid}">{escape(version)}</a></li>')
    if not parts:
        raise BuildError("CHANGELOG.md holds no released version")
    toc_html = "    <ol>\n" + "\n".join(toc) + "\n    </ol>"
    lede = (
        "    <h1>Changelog</h1>\n"
        f"    <p>What changed in each release of moai, newest first — <code>CHANGELOG.md</code> as of "
        f"{escape(release_label(ref))}. Releases and their files are on "
        f'<a href="{REPO_URL}/releases">GitHub</a>.</p>'
    )
    return toc_html, lede + "\n" + "\n".join(parts)


def write(pages: dict[Path, str], *, check: bool) -> list[Path]:
    stale = []
    for rel_path, text in sorted(pages.items()):
        path = SITE / rel_path
        old = path.read_text(encoding="utf-8") if path.exists() else None
        if old == text:
            continue
        stale.append(rel_path)
        if not check:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text, encoding="utf-8")
    return stale


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(
        description=__doc__.split("\n\n")[0], formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("--check", action="store_true", help="write nothing; exit 1 if a committed page is stale")
    ap.add_argument("--docs", type=Path, metavar="DIR", help="also build the manual (site/docs/) from this wiki directory")
    ap.add_argument("--changelog", type=Path, metavar="FILE", help="also build the changelog (site/changelog/) from this file")
    ap.add_argument("--ref", default="develop", help="the release the manual is read from (links and the label)")
    args = ap.parse_args(argv)
    try:
        pages = build_pages()
        release = build_release_pages(args.docs, args.changelog, args.ref)
    except BuildError as e:
        print(f"gen.py: {e}", file=sys.stderr)
        return 2
    stale = write(pages, check=args.check)
    if args.check:
        for p in stale:
            print(f"site/{p} is stale — run python3 scripts/site/gen.py and commit it", file=sys.stderr)
        return 1 if stale else 0
    for p in stale + write(release, check=False):
        print(f"wrote site/{p}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
