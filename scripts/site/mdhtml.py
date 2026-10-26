"""A small, safe Markdown -> HTML renderer (Python 3.11+, standard library only).

A port of web/panel/view/markdown.ts (the renderer `moai web` uses for wiki pages): the same block pass
(fences, indented code, headings, lists, tables, quotes, paragraphs) and the same inline pass (code spans,
escapes, entities, links, reference links, emphasis, strikethrough). Where the TS builds DOM nodes this returns
an HTML string, and everything that reaches the output is escaped: raw HTML stays literal text, images are never
emitted (their alt text is), and a link whose scheme is not http, https or mailto is drawn as plain text.

Changes to the rules here should be mirrored in markdown.ts (and the other way round); the anchor rule also
lives in src/wiki.rs `Anchors`.
"""

from __future__ import annotations

import re
import unicodedata
from typing import Callable, NamedTuple


class Heading(NamedTuple):
    level: int
    text: str
    anchor: str


# ---- escaping ----

def esc(s: str) -> str:
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def esc_attr(s: str) -> str:
    return esc(s).replace('"', "&quot;")


# ---- anchors ----

def _keep_for_anchor(ch: str) -> bool:
    if ch in "- ":
        return True
    if ch.isalpha():
        return True
    cat = unicodedata.category(ch)
    return cat[0] == "M" or cat in ("Nd", "Nl", "Pc")


def github_anchor(text: str, seen: dict[str, int] | None = None) -> str:
    """GitHub slug rule: trim, lowercase, keep letters/marks/digits/'-'/'_'/space, space -> '-'.
    With `seen`, a repeat gets '-1', '-2', ... (same as the TS `githubAnchor`)."""
    base = "".join(c for c in text.strip().lower() if _keep_for_anchor(c)).replace(" ", "-")
    if seen is None:
        return base
    slug = base
    while slug in seen:
        count = seen.get(base, 0) + 1
        seen[base] = count
        slug = f"{base}-{count}"
    seen[slug] = 0
    return slug


# ---- context ----

class _Ctx:
    def __init__(self, resolve: Callable[[str], str]) -> None:
        self.resolve = resolve
        self.headings: list[Heading] = []
        self.seen: dict[str, int] = {}
        self.refs: dict[str, str] = {}


class _Buf:
    """Inline output: the html, and the plain text (what a heading's anchor is made from)."""

    def __init__(self) -> None:
        self.html: list[str] = []
        self.text: list[str] = []

    def add_text(self, s: str) -> None:
        self.html.append(esc(s))
        self.text.append(s)

    def add_html(self, h: str, t: str = "") -> None:
        self.html.append(h)
        if t:
            self.text.append(t)

    def merge(self, other: "_Buf") -> None:
        self.html.extend(other.html)
        self.text.extend(other.text)

    def get_html(self) -> str:
        return "".join(self.html)

    def get_text(self) -> str:
        return "".join(self.text)


def _ch(s: str, i: int) -> str:
    return s[i] if 0 <= i < len(s) else ""


def _space(ch: str) -> bool:
    return ch != "" and re.match(r"\s", ch) is not None


def _word(ch: str) -> bool:
    return ch != "" and ch.isalnum()


# ---- entry ----

def render(source: str, resolve_link: Callable[[str], str] | None = None) -> tuple[str, list[Heading]]:
    """Return (html, headings). `resolve_link` maps a link's href as written to the href to emit."""
    ctx = _Ctx(resolve_link or (lambda h: h))
    text = source or ""
    if text.startswith("\ufeff"):
        text = text[1:]
    text = text.replace("\r\n", "\n").replace("\r", "\n")
    lines = [
        re.sub(r"^[ \t]*\t[ \t]*", lambda m: m.group(0).replace("\t", "    "), line)
        for line in text.split("\n")
    ]
    out: list[tuple[bool, str]] = []
    _parse_blocks(ctx, _collect_definitions(ctx, lines), out, False)
    return "\n".join(h for _, h in out), ctx.headings


# ---- reference definitions ----

_REF_DEF = re.compile(
    r"""^ {0,3}\[((?:[^\[\]\\]|\\.)+)\]:[ \t]*(?:<([^<>\n]*)>|([^\s<]\S*))(?:[ \t]+(?:"[^"]*"|'[^']*'|\([^()]*\)))?[ \t]*$"""
)


def _ref_label(label: str) -> str:
    return re.sub(r"\s+", " ", label.strip()).lower()


def _fence_close(mark: str) -> re.Pattern[str]:
    c = "`" if mark[0] == "`" else "~"
    return re.compile(rf"^ {{0,3}}{c}{{{len(mark)},}}[ \t]*$")


def _collect_definitions(ctx: _Ctx, lines: list[str]) -> list[str]:
    out: list[str] = []
    fence: re.Pattern[str] | None = None
    for line in lines:
        if fence is not None:
            if fence.match(line):
                fence = None
            out.append(line)
            continue
        op = _FENCE.match(line)
        if op and _is_fence(line):
            fence = _fence_close(op.group(2))
            out.append(line)
            continue
        d = _REF_DEF.match(line)
        label = _ref_label(d.group(1)) if d else ""
        dest = (d.group(2) if d.group(2) is not None else (d.group(3) or "")) if d else ""
        if d and label != "" and dest != "":
            if label not in ctx.refs:
                ctx.refs[label] = _decode_entities(re.sub(r"\\([!-/:-@\[-`{-~])", r"\1", dest))
            continue
        out.append(line)
    return out


# ---- block pass ----

_FENCE = re.compile(r"^( {0,3})(`{3,}|~{3,})(.*)$")
_ATX = re.compile(r"^ {0,3}(#{1,6})(?:[ \t]+(.*?))?(?:[ \t]+#+)?[ \t]*$")
_SETEXT = re.compile(r"^ {0,3}(=+|-+)[ \t]*$")
_HR = re.compile(r"^ {0,3}([-*_])(?:[ \t]*\1){2,}[ \t]*$")
_QUOTE = re.compile(r"^ {0,3}>")
_LIST = re.compile(r"^( *)([-+*]|\d{1,9}[.)])( +|$)")
_DELIM_CELL = re.compile(r"^:?-+:?$")


def _is_blank(line: str) -> bool:
    return line.strip() == ""


def _indent_of(line: str) -> int:
    return len(line) - len(line.lstrip())


def _is_fence(line: str) -> bool:
    m = _FENCE.match(line)
    return bool(m) and not (m.group(2)[0] == "`" and "`" in m.group(3))


def _interrupts(line: str) -> bool:
    if _is_fence(line) or _ATX.match(line) or _HR.match(line) or _QUOTE.match(line):
        return True
    m = _LIST.match(line)
    if m and len(m.group(1)) < 4 and m.group(3) != "":
        return not m.group(2)[0].isdigit() or int(re.match(r"\d+", m.group(2)).group(0)) == 1  # type: ignore[union-attr]
    return False


def _split_row(line: str) -> list[str]:
    s = line.strip()
    if s.startswith("|"):
        s = s[1:]
    if s.endswith("|") and not s.endswith("\\|"):
        s = s[:-1]
    cells: list[str] = []
    cell = ""
    i = 0
    while i < len(s):
        if s[i] == "\\" and _ch(s, i + 1) == "|":
            cell += "|"
            i += 1
        elif s[i] == "|":
            cells.append(cell.strip())
            cell = ""
        else:
            cell += s[i]
        i += 1
    cells.append(cell.strip())
    return cells


def _table_at(lines: list[str], i: int) -> tuple[list[str], list[str]] | None:
    if i + 1 >= len(lines):
        return None
    nxt = lines[i + 1]
    if "|" not in lines[i] or "-" not in nxt:
        return None
    delim = _split_row(nxt)
    if not all(_DELIM_CELL.match(c) for c in delim):
        return None
    head = _split_row(lines[i])
    if len(head) != len(delim):
        return None
    aligns = [
        ("center" if c.endswith(":") else "left") if c.startswith(":") else ("right" if c.endswith(":") else "")
        for c in delim
    ]
    return head, aligns


def _inline_html(ctx: _Ctx, s: str) -> _Buf:
    buf = _Buf()
    _inline_into(ctx, buf, s, False)
    return buf


def _heading(ctx: _Ctx, out: list[tuple[bool, str]], level: int, source: str) -> None:
    buf = _inline_html(ctx, source.strip())
    text = buf.get_text()
    anchor = github_anchor(text, ctx.seen)
    ctx.headings.append(Heading(level, text, anchor))
    out.append((True, f'<h{level} id="{esc_attr(anchor)}">{buf.get_html()}</h{level}>'))


def _compose(pieces: list[tuple[bool, str]]) -> str:
    res = ""
    prev_block = False
    for blk, h in pieces:
        if res and (blk or prev_block):
            res += "\n"
        res += h
        prev_block = blk
    return res


def _align_attr(align: str) -> str:
    return f' style="text-align:{align}"' if align in ("center", "right") else ""


def _parse_blocks(ctx: _Ctx, lines: list[str], out: list[tuple[bool, str]], tight: bool) -> None:
    n = len(lines)
    i = 0
    while i < n:
        line = lines[i]
        if _is_blank(line):
            i += 1
            continue

        # Fenced code.
        fence = _FENCE.match(line)
        if fence and _is_fence(line):
            indent = len(fence.group(1))
            close = _fence_close(fence.group(2))
            strip = re.compile(rf"^ {{0,{indent}}}")
            body: list[str] = []
            i += 1
            while i < n and not close.match(lines[i]):
                body.append(strip.sub("", lines[i], count=1))
                i += 1
            i += 1
            words = fence.group(3).strip().split()
            info = words[0] if words else ""
            cls = f' class="language-{esc_attr(info)}"' if info else ""
            out.append((True, f"<pre><code{cls}>{esc(chr(10).join(body))}</code></pre>"))
            continue

        # Indented code.
        if line.startswith("    "):
            body = []
            while i < n and (
                lines[i].startswith("    ")
                or (_is_blank(lines[i]) and i + 1 < n and (lines[i + 1].startswith("    ") or _is_blank(lines[i + 1])))
            ):
                body.append(lines[i][4:])
                i += 1
            while body and _is_blank(body[-1]):
                body.pop()
            out.append((True, f"<pre><code>{esc(chr(10).join(body))}</code></pre>"))
            continue

        # Heading.
        atx = _ATX.match(line)
        if atx:
            _heading(ctx, out, len(atx.group(1)), atx.group(2) or "")
            i += 1
            continue

        if _HR.match(line):
            out.append((True, "<hr>"))
            i += 1
            continue

        # Block quote.
        if _QUOTE.match(line):
            inner: list[str] = []
            quote_fence: re.Pattern[str] | None = None
            while i < n and not _is_blank(lines[i]):
                if _QUOTE.match(lines[i]):
                    text = re.sub(r"^ {0,3}> ?", "", lines[i], count=1)
                    op = _FENCE.match(text)
                    if quote_fence is not None:
                        if quote_fence.match(text):
                            quote_fence = None
                    elif op and _is_fence(text):
                        quote_fence = _fence_close(op.group(2))
                    inner.append(text)
                elif quote_fence is not None or _interrupts(lines[i]):
                    break
                else:
                    inner.append(lines[i])
                i += 1
            sub: list[tuple[bool, str]] = []
            _parse_blocks(ctx, inner, sub, False)
            inside = "\n".join(h for _, h in sub)
            out.append((True, f"<blockquote>\n{inside}\n</blockquote>" if inside else "<blockquote></blockquote>"))
            continue

        # List.
        first = _LIST.match(line)
        if first and len(first.group(1)) < 4:
            i = _parse_list(ctx, lines, i, out)
            continue

        # Table.
        table = _table_at(lines, i)
        if table:
            head, aligns = table
            cols = len(head)
            ths = "".join(
                f"<th{_align_attr(aligns[c])}>{_inline_html(ctx, cell).get_html()}</th>" for c, cell in enumerate(head)
            )
            rows: list[str] = []
            i += 2
            while i < n and not _is_blank(lines[i]) and not _interrupts(lines[i]):
                cells = _split_row(lines[i])
                tds = "".join(
                    f"<td{_align_attr(aligns[c])}>{_inline_html(ctx, cells[c] if c < len(cells) else '').get_html()}</td>"
                    for c in range(cols)
                )
                rows.append(f"<tr>{tds}</tr>")
                i += 1
            tbody = f"<tbody>\n{chr(10).join(rows)}\n</tbody>" if rows else "<tbody></tbody>"
            out.append(
                (True, f'<div class="table"><table>\n<thead><tr>{ths}</tr></thead>\n{tbody}\n</table></div>')
            )
            continue

        # Paragraph, or a setext heading.
        para = [line.lstrip()]
        setext = 0
        i += 1
        while i < n and not _is_blank(lines[i]):
            under = _SETEXT.match(lines[i])
            if under:
                setext = 1 if under.group(1)[0] == "=" else 2
                i += 1
                break
            if _interrupts(lines[i]) or _table_at(lines, i):
                break
            para.append(lines[i].lstrip())
            i += 1
        text = re.sub(r"\s+$", "", "\n".join(para))
        if setext:
            _heading(ctx, out, setext, text)
        elif tight and not (out and not out[-1][0]):
            out.append((False, _inline_html(ctx, text).get_html()))
        else:
            out.append((True, f"<p>{_inline_html(ctx, text).get_html()}</p>"))


def _parse_list(ctx: _Ctx, lines: list[str], start: int, out: list[tuple[bool, str]]) -> int:
    n = len(lines)
    head = _LIST.match(lines[start])
    assert head is not None
    ordered = head.group(2)[0].isdigit()
    delim = head.group(2)[-1]

    def same_kind(line: str) -> re.Match[str] | None:
        m = _LIST.match(line)
        if not m or len(m.group(1)) >= 4 or _HR.match(line):
            return None
        if m.group(2)[0].isdigit() != ordered or m.group(2)[-1] != delim:
            return None
        return m

    items: list[list[str]] = []
    loose = False
    i = start
    while i < n:
        m = same_kind(lines[i])
        if not m:
            break
        gap = len(m.group(3)) if 1 <= len(m.group(3)) <= 4 else 1
        content_indent = len(m.group(1)) + len(m.group(2)) + gap
        item = ["" if m.group(3) == "" else lines[i][content_indent:]]
        i += 1
        while i < n:
            l = lines[i]
            if _is_blank(l):
                j = i
                while j < n and _is_blank(lines[j]):
                    j += 1
                if j < n and _indent_of(lines[j]) >= content_indent:
                    while i < j:
                        item.append("")
                        i += 1
                    continue
                break
            if _indent_of(l) >= content_indent:
                item.append(l[content_indent:])
                i += 1
                continue
            if not _is_blank(item[-1]) and not _interrupts(l) and not _LIST.match(l) and not _table_at(lines, i):
                item.append(l.lstrip())
                i += 1
                continue
            break
        blank_between = any(
            k > 0 and _is_blank(x) and not _is_blank(item[k - 1]) and any(not _is_blank(y) for y in item[k:])
            for k, x in enumerate(item)
        )
        if blank_between and not any(_LIST.match(x) for x in item):
            loose = True
        while item and _is_blank(item[-1]):
            item.pop()
        items.append(item)
        j = i
        while j < n and _is_blank(lines[j]):
            j += 1
        if j < n and same_kind(lines[j]):
            if j > i:
                loose = True
            i = j
            continue
        break

    tag = "ol" if ordered else "ul"
    first_num = int(re.match(r"\d+", head.group(2)).group(0)) if ordered else 1  # type: ignore[union-attr]
    open_tag = f'<ol start="{first_num}">' if ordered and first_num != 1 else f"<{tag}>"
    lis: list[str] = []
    for item in items:
        pieces: list[tuple[bool, str]] = []
        _parse_blocks(ctx, item, pieces, not loose)
        lis.append(f"<li>{_compose(pieces)}</li>")
    out.append((True, open_tag + "\n" + "\n".join(lis) + f"\n</{tag}>"))
    return i


# ---- inline pass ----

_PUNCT = re.compile(r"[!-/:-@\[-`{-~]")
_AUTOLINK = re.compile(r"<([a-zA-Z][a-zA-Z0-9+.-]{1,31}:[^\s<>]*)>")

_ENTITIES = {
    "amp": "&", "lt": "<", "gt": ">", "quot": '"', "apos": "'", "nbsp": "\u00a0", "copy": "\u00a9",
    "reg": "\u00ae", "trade": "\u2122", "mdash": "\u2014", "ndash": "\u2013", "hellip": "\u2026",
    "laquo": "\u00ab", "raquo": "\u00bb", "middot": "\u00b7", "bull": "\u2022", "rarr": "\u2192",
    "larr": "\u2190", "times": "\u00d7",
}
_ENTITY = re.compile(r"&(?:#([0-9]{1,7})|#[xX]([0-9a-fA-F]{1,6})|([a-zA-Z][a-zA-Z0-9]{1,8}));")


def _entity_at(s: str, i: int) -> tuple[str, int] | None:
    m = _ENTITY.match(s, i)
    if not m:
        return None
    length = len(m.group(0))
    if m.group(3) is not None:
        named = _ENTITIES.get(m.group(3))
        return None if named is None else (named, length)
    cp = int(m.group(1), 10) if m.group(1) is not None else int(m.group(2) or "0", 16)
    control = (cp != 9 and cp != 10) if cp < 0x20 else (0x7F <= cp <= 0x9F)
    if cp == 0 or control or 0xD800 <= cp <= 0xDFFF or cp > 0x10FFFF:
        return None
    return chr(cp), length


def _decode_entities(s: str) -> str:
    if "&" not in s:
        return s
    out: list[str] = []
    i = 0
    while i < len(s):
        e = _entity_at(s, i) if s[i] == "&" else None
        if e:
            out.append(e[0])
            i += e[1]
        else:
            out.append(s[i])
            i += 1
    return "".join(out)


_SAFE_SCHEMES = ("http", "https", "mailto")


def _scheme_ok(href: str) -> bool:
    """A href with a scheme must be http, https or mailto; scheme-less (relative, #anchor) hrefs pass."""
    cleaned = re.sub(r"[\x00-\x20\x7f-\x9f\u200b-\u200f\u2028\u2029\ufeff]", "", href)
    m = re.match(r"([a-zA-Z][a-zA-Z0-9+.-]*):", cleaned)
    return m is None or m.group(1).lower() in _SAFE_SCHEMES


def _make_link(ctx: _Ctx, href: str) -> str | None:
    """The opening <a ...> tag for `href`, or None when the link must be drawn as plain text."""
    if not _scheme_ok(href):
        return None
    try:
        target = ctx.resolve(href)
    except Exception:
        target = href
    if not isinstance(target, str) or not _scheme_ok(target):
        return None
    rel = ' rel="noopener"' if re.match(r"https?://", target, re.I) else ""
    return f'<a href="{esc_attr(target)}"{rel}>'


def _code_span_end(s: str, i: int) -> tuple[int, int]:
    run = 1
    while _ch(s, i + run) == "`":
        run += 1
    j = i + run
    while j < len(s):
        k = s.find("`", j)
        if k < 0:
            break
        r = 1
        while _ch(s, k + r) == "`":
            r += 1
        if r == run:
            return run, k
        j = k + r
    return run, -1


def _bracket_end(s: str, i: int) -> int:
    depth = 1
    j = i + 1
    while j < len(s):
        c = s[j]
        if c == "\\":
            j += 2
            continue
        if c == "`":
            run, end = _code_span_end(s, j)
            j = end + run if end >= 0 else j + run
            continue
        if c == "[":
            depth += 1
        elif c == "]":
            depth -= 1
            if depth == 0:
                break
        j += 1
    return j if depth == 0 else -1


def _parse_link_at(s: str, i: int) -> tuple[str, str, int] | None:
    j = _bracket_end(s, i)
    if j < 0 or _ch(s, j + 1) != "(":
        return None
    text = s[i + 1 : j]
    k = j + 2
    while _ch(s, k) in (" ", "\n") and k < len(s):
        k += 1
    dest = ""
    if _ch(s, k) == "<":
        close = s.find(">", k)
        if close < 0 or "\n" in s[k:close]:
            return None
        dest = s[k + 1 : close]
        k = close + 1
    else:
        parens = 0
        while k < len(s):
            c = s[k]
            if c == "\\" and k + 1 < len(s) and _PUNCT.match(s[k + 1]):
                k += 1
                dest += s[k]
                k += 1
                continue
            if re.match(r"\s", c):
                break
            if c == "(":
                parens += 1
            elif c == ")":
                if parens == 0:
                    break
                parens -= 1
            dest += c
            k += 1
    while _ch(s, k) in (" ", "\n") and k < len(s):
        k += 1
    q = _ch(s, k)
    if q in ('"', "'", "("):
        closer = ")" if q == "(" else q
        t = k + 1
        while t < len(s) and s[t] != closer:
            t += 2 if s[t] == "\\" else 1
        if t >= len(s):
            return None
        k = t + 1
        while _ch(s, k) in (" ", "\n") and k < len(s):
            k += 1
    if _ch(s, k) != ")" or dest == "":
        return None
    return text, _decode_entities(dest), k + 1


def _parse_ref_at(ctx: _Ctx, s: str, i: int) -> tuple[str, str, int] | None:
    if not ctx.refs:
        return None
    j = _bracket_end(s, i)
    if j < 0:
        return None
    text = s[i + 1 : j]
    if _ch(s, j + 1) == "[":
        k = s.find("]", j + 2)
        inner = None if k < 0 else s[j + 2 : k]
        if inner is not None and "[" not in inner:
            dest = ctx.refs.get(_ref_label(text if inner == "" else inner))
            if dest is not None:
                return text, dest, k + 1
    dest = ctx.refs.get(_ref_label(text))
    return None if dest is None or text.strip() == "" else (text, dest, j + 1)


class _EmphMemo:
    def __init__(self, last_close: dict[str, int]) -> None:
        self.memo: dict[int, tuple[int, int, int, int, int] | None] = {}
        self.last_close = last_close


def _match_emph(s: str, i: int, memo: _EmphMemo) -> tuple[int, int, int, int, int] | None:
    """The emphasis opening at i as (lit, k, start, close, end), or None. Memoized per position."""
    if i in memo.memo:
        return memo.memo[i]
    found = _find_emph(s, i, memo)
    memo.memo[i] = found
    return found


def _find_emph(s: str, i: int, memo: _EmphMemo) -> tuple[int, int, int, int, int] | None:
    c = s[i]
    r = 1
    while _ch(s, i + r) == c:
        r += 1
    before = _ch(s, i - 1)
    after = _ch(s, i + r)
    if _space(after):
        return None
    if c == "_" and _word(before):
        return None
    if memo.last_close.get(c, float("inf")) < i + r:
        return None
    sizes = [3, 2, 1] if r >= 3 else [2, 1] if r == 2 else [1]
    for k in sizes:
        j = i + r
        while j < len(s):
            ch = s[j]
            if ch == "\\":
                j += 2
                continue
            if ch == "`":
                run, end = _code_span_end(s, j)
                j = end + run if end >= 0 else j + run
                continue
            if ch != c:
                j += 1
                continue
            rr = 1
            while _ch(s, j + rr) == c:
                rr += 1
            prev = _ch(s, j - 1)
            nxt = _ch(s, j + rr)
            can_close = not _space(prev) and (c != "_" or not _word(nxt))
            can_open = not _space(nxt) and (c != "_" or not _word(prev))
            if can_close and rr >= k:
                return (r - k, k, i + r, j, j + k)
            if can_open and not can_close:
                nested = _match_emph(s, j, memo)
                if nested:
                    j = nested[4]
                    continue
            j += rr
    return None


def _inline_into(ctx: _Ctx, parent: _Buf, s: str, no_links: bool) -> None:
    last_close = {"*": -1, "_": -1}
    for p in range(1, len(s)):
        ch = s[p]
        prev = s[p - 1]
        if ch in ("*", "_") and not _space(prev) and (prev != ch or _ch(s, p - 2) == "\\"):
            last_close[ch] = p
    memo = _EmphMemo(last_close)
    for p in range(len(s) - 1, -1, -1):
        if s[p] in ("*", "_"):
            _match_emph(s, p, memo)

    strike_fail = float("inf")
    buf = ""

    def flush() -> None:
        nonlocal buf
        if buf:
            parent.add_text(buf)
        buf = ""

    i = 0
    while i < len(s):
        c = s[i]
        if c == "\\":
            nx = _ch(s, i + 1)
            if nx == "\n":
                flush()
                parent.add_html("<br>")
                i += 2
                continue
            if nx != "" and _PUNCT.match(nx):
                buf += nx
                i += 2
                continue
            buf += c
            i += 1
            continue
        if c == "\n":
            if re.search(r" {2,}$", buf):
                buf = buf.rstrip(" ")
                flush()
                parent.add_html("<br>")
            else:
                buf = buf.rstrip(" ") + "\n"
            i += 1
            continue
        if c == "`":
            run, end = _code_span_end(s, i)
            if end >= 0:
                body = s[i + run : end].replace("\n", " ")
                if len(body) > 2 and body.startswith(" ") and body.endswith(" ") and body.strip() != "":
                    body = body[1:-1]
                flush()
                parent.add_html(f"<code>{esc(body)}</code>", body)
                i = end + run
            else:
                buf += "`" * run
                i += run
            continue
        if c == "!" and _ch(s, i + 1) == "[":
            m = None if no_links else _parse_link_at(s, i + 1)
            if m:
                flush()
                parent.add_text(s[i : m[2]])
                i = m[2]
            else:
                buf += c
                i += 1
            continue
        if c == "[" and not no_links:
            m = _parse_link_at(s, i) or _parse_ref_at(ctx, s, i)
            if m:
                text, dest, end = m
                flush()
                sub = _Buf()
                _inline_into(ctx, sub, text, True)
                if not sub.html:
                    sub.add_text(dest)
                tag = _make_link(ctx, dest)
                if tag is None:
                    parent.merge(sub)
                else:
                    parent.add_html(tag + sub.get_html() + "</a>", sub.get_text())
                i = end
                continue
        if c == "<":
            m = _AUTOLINK.match(s, i)
            if m:
                flush()
                tag = None if no_links else _make_link(ctx, m.group(1))
                if tag is None:
                    parent.add_text(m.group(0))
                else:
                    parent.add_html(tag + esc(m.group(1)) + "</a>", m.group(1))
                i += len(m.group(0))
                continue
        if c == "&":
            e = _entity_at(s, i)
            if e:
                buf += e[0]
                i += e[1]
                continue
        if c == "~" and _ch(s, i + 1) == "~" and _ch(s, i + 2) != "~" and i < strike_fail:
            opn = _ch(s, i + 2)
            close = -1
            if opn != "" and not _space(opn):
                k = s.find("~~", i + 2)
                while k >= 0:
                    if k > i + 2 and _ch(s, k + 2) != "~" and _ch(s, k - 1) != "~" and not _space(_ch(s, k - 1)):
                        close = k
                        break
                    k = s.find("~~", k + 1)
            if close >= 0:
                flush()
                sub = _Buf()
                _inline_into(ctx, sub, s[i + 2 : close], no_links)
                parent.add_html("<del>" + sub.get_html() + "</del>", sub.get_text())
                i = close + 2
                continue
            strike_fail = i
        if c in ("*", "_"):
            m = _match_emph(s, i, memo)
            if m:
                lit, k, start, close, end = m
                buf += c * lit
                flush()
                tags = ["strong", "em"] if k == 3 else ["strong"] if k == 2 else ["em"]
                sub = _Buf()
                _inline_into(ctx, sub, s[start:close], no_links)
                h = sub.get_html()
                for tag in reversed(tags):
                    h = f"<{tag}>{h}</{tag}>"
                parent.add_html(h, sub.get_text())
                i = end
                continue
            r = 1
            while _ch(s, i + r) == c:
                r += 1
            buf += c * r
            i += r
            continue
        buf += c
        i += 1
    flush()
