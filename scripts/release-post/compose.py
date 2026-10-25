#!/usr/bin/env python3
"""CHANGELOG 의 한 판을 Discord 에 올릴 글 한 통으로 줄인다.

    scripts/release-post/compose.py v0.23.0                      글을 찍는다
    scripts/release-post/compose.py v0.23.0 --out release-posts/v0.23.0   post.md 로 쓴다

글은 큰 변경(Added·Changed·Deprecated·Removed) → 버그 수정(Fixed) → 보안(Security) 차례다. 항목마다
첫 문장만 남긴다 — 자세한 것은 릴리스 노트가 들고, 이 글은 무엇이 달라졌는지를 알리는 데서 그친다
(2026-10-25 사용자 결정, moai-hwl4). 굵은 머리말로 시작하는 항목은 그 머리말이 곧 요약이다.

머리줄은 release.yml 의 "노트를 고른다" 와 같은 규칙으로 찾는다 — 정규식이 아니라 앞머리 비교다.
판에 든 `.` 하나가 아무 글자나 맞히면 엉뚱한 절을 집는다.
"""
import argparse
import os
import re
import sys

DISCORD_LIMIT = 2000  # Discord 메시지 한 통의 글자 상한
ITEM_CHARS = 160  # 항목 하나의 상한. 넘으면 낱말 경계에서 자르고 … 를 단다
MAX_ITEMS = 6  # 갈래마다 싣는 항목 수. 나머지는 "…and N more" 한 줄로 릴리스 노트에 넘긴다
SITE = "https://moai.buzzni.com"

BIG = ("Added", "Changed", "Deprecated", "Removed")
KNOWN = BIG + ("Fixed", "Security")

# `(moai-pxiy)`, `(moai-ab12, moai-cd34.x9z)` — 트래커 id 만 든 괄호. Discord 에서 읽는 사람에게는 뜻이 없다.
# 접두는 이 저장소의 것(`moai-`)만 본다 — `(read-only)`·`(per-user)` 도 "낱말-네 글자" 꼴이라, 아무 접두나 받으면
# 본문의 낱말이 id 로 읽혀 깎인다
IDS = re.compile(r"\s*\((?:moai-[0-9a-z]{4}(?:\.[0-9a-z]{3})*(?:\s*[,·]\s*|\s+)?)+\)")
# 화면(탐색기·moai web)을 바꾼 항목. 이런 항목이 든 판의 소식에는 그림이 있어야 한다(2026-10-25 사용자 결정,
# moai-r96m) — v0.24.0 이 새 간트를 글로만 올렸다. 낱말로 어림하니 `moai tui --json` 같은 것도 걸리지만, 알림
# 한 줄이 헛도는 값이 그림 없이 나가는 값보다 싸다
SCREEN = re.compile(r"\bexplorer\b|`moai (?:tui|web)\b|`SPC ", re.IGNORECASE)
# 마침표가 문장 끝이 아닌 줄임말 — `e.g. ` 에서 끊으면 요약이 "Use e.g" 가 된다
ABBREV = re.compile(r"(?:^|[\s(])(?:e\.g|i\.e|vs)\.$", re.IGNORECASE)


class NoRelease(Exception):
    """CHANGELOG 에 그 판의 절이 없다."""


def bare(version):
    return version[1:] if version.startswith("v") else version


def section(changelog, version):
    """`## [<판>]` 절의 몸을 돌려준다. 절이 없으면 None, 있는데 비었으면 빈 글이다."""
    head = f"## [{bare(version)}]"
    found, on, body = False, False, []
    for line in changelog.splitlines():
        if line.startswith("## ["):
            on = line.startswith(head)
            found = found or on
            continue
        if on:
            body.append(line)
    return "\n".join(body) if found else None


def parse(body):
    """`### 절` 마다 항목 목록. 이어지는 들여 쓴 줄은 그 항목에 접는다."""
    parts, heading, item = {}, None, None

    def close():
        nonlocal item
        if heading is not None and item is not None:
            parts.setdefault(heading, []).append(" ".join(item))
        item = None

    for line in body.splitlines():
        if line.startswith("### "):
            close()
            heading = line[4:].strip()
        elif line.startswith("- ") and heading is not None:
            close()
            item = [line[2:].strip()]
        elif item is not None and line.strip() and line[:1].isspace():
            item.append(line.strip())
        elif not line.strip():
            close()
    close()
    return parts


def first_sentence(text):
    """첫 문장. 코드(`…`) 안의 마침표와 `0.23.0` 처럼 빈칸이 안 따르는 마침표는 끝으로 안 읽는다."""
    code = False
    for i, ch in enumerate(text):
        if ch == "`":
            code = not code
        elif ch in ".!?" and not code and (i + 1 == len(text) or text[i + 1] == " "):
            if ABBREV.search(text[: i + 1]):
                continue
            return text[:i]
    return text


def drop_aside(text):
    """콜론·쌍반점·대시 뒤의 덧붙임과 끝의 괄호를 걷는다 — 무엇이 달라졌는지만 남기려는 것이다.

    머리가 너무 짧아지는 자리(`Note: …`)에서는 안 자른다. 코드 안의 `:` 는 덧붙임이 아니다.
    """
    text = re.sub(r"\s*\([^()]*\)\s*$", "", text)
    code = False
    for i, ch in enumerate(text):
        if ch == "`":
            code = not code
        elif not code and i >= 20 and text.startswith((": ", "; ", " — "), i):
            return text[:i]
    return text


def balance(text):
    """끊은 자리가 코드(`) 나 굵게(**) 안이면 닫는다 — 열린 채 두면 Discord 가 표식을 글자 그대로 찍는다."""
    if text.count("`") % 2:
        text += "`"
    if re.sub(r"`[^`]*`", "", text).count("**") % 2:
        text += "**"
    return text


def cut(text, limit=ITEM_CHARS):
    """상한 안으로 줄인다. 덧붙임을 걷거나 꼬리를 자른 자리에서 열린 표식은 닫는다."""
    text = balance(text)
    if len(text) <= limit:
        return text
    head = text[: limit - 4]  # … 하나와, 닫을 표식(` 하나·** 하나) 셋의 자리를 남긴다
    space = head.rfind(" ")
    return balance((head[:space] if space > 0 else head).rstrip(" ,;:—-") + "…")


def summarize(item):
    """항목 하나를 한 줄로. 굵은 머리말이 문장으로 끝나면 그 머리말만, 아니면 첫 문장이다."""
    text = IDS.sub("", item).strip()
    bold = re.match(r"\*\*(.+?)\*\*", text)
    if bold and bold.group(1).rstrip().endswith((".", "!", "?")):
        return cut("**" + bold.group(1).rstrip().rstrip(".") + "**")
    return cut(drop_aside(first_sentence(text).strip()))


def render(version, groups, dropped, repo):
    v = "v" + bare(version)
    out = [f"## moai {v}"]
    for title, items in groups:
        if not items:
            continue
        out.append(f"### {title}")
        out.extend(f"- {line}" for line in items)
        if dropped.get(title):
            out.append(f"- …and {dropped[title]} more in the release notes")
    if not any(items for _, items in groups):
        out.append("This release lists no changes of its own.")
    out.append("")
    out.append(f"Release notes: <https://github.com/{repo}/releases/tag/{v}>")
    out.append(f"Update with `moai update` · install: <{SITE}/guide/#install>")
    return "\n".join(out)


def compose(changelog, version, repo):
    body = section(changelog, version)
    if body is None:
        raise NoRelease(f"CHANGELOG has no `## [{bare(version)}]` section")
    parts = parse(body)
    groups = [
        ("What's new", [summarize(i) for h in BIG for i in parts.get(h, [])]),
        ("Fixes", [summarize(i) for i in parts.get("Fixed", [])]),
        ("Security", [summarize(i) for i in parts.get("Security", [])]),
    ]
    dropped = {}
    for title, items in groups:
        if len(items) > MAX_ITEMS:
            dropped[title] = len(items) - MAX_ITEMS
            del items[MAX_ITEMS:]
    # 한 통에 안 들면 가장 긴 갈래의 꼬리부터 덜어 낸다. 갈래마다 하나는 남긴다 — 버그만 통째로 사라지지 않게
    while len(render(version, groups, dropped, repo)) > DISCORD_LIMIT:
        title, items = max(groups, key=lambda g: len(g[1]))
        if len(items) <= 1:
            break
        items.pop()
        dropped[title] = dropped.get(title, 0) + 1
    return render(version, groups, dropped, repo)


def screen_changes(changelog, version):
    """큰 변경(Added·Changed·Deprecated·Removed) 가운데 화면을 바꾼 항목의 수."""
    parts = parse(section(changelog, version) or "")
    return sum(1 for h in BIG for i in parts.get(h, []) if SCREEN.search(i))


def has_media(directory):
    """post.py 가 그림 메시지로 보낼 파일이 media/ 에 하나라도 있는가 (글 파일 `.md` 는 안 센다)."""
    media = os.path.join(directory, "media")
    if not os.path.isdir(media):
        return False
    return any(not n.startswith(".") and not n.endswith(".md") for n in os.listdir(media))


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("version", help="v0.23.0 or 0.23.0")
    ap.add_argument("--changelog", default="CHANGELOG.md")
    ap.add_argument("--repo", default="buzzni/moai", help="owner/name for the release link")
    ap.add_argument("--out", help="write <dir>/post.md instead of printing")
    ap.add_argument("--force", action="store_true", help="overwrite an existing post.md")
    args = ap.parse_args(argv)
    with open(args.changelog, encoding="utf-8") as f:
        changelog = f.read()
    try:
        post = compose(changelog, args.version, args.repo)
    except NoRelease as e:
        print(f"compose.py: {e}", file=sys.stderr)
        return 1
    unknown = sorted(set(parse(section(changelog, args.version))) - set(KNOWN))
    if unknown:
        print(f"compose.py: left out sections it does not know: {', '.join(unknown)}", file=sys.stderr)
    # 알리기만 한다 — 종료 코드는 그대로다. 그림이 없는 소식도 나가야 할 때가 있고, 막는 순간 이것이 게이트가 된다
    directory = args.out or os.path.join("release-posts", "v" + bare(args.version))
    screens = screen_changes(changelog, args.version)
    if screens and not has_media(directory):
        print(
            f"compose.py: {screens} change(s) in this release touch the explorer or `moai web`, "
            f"and {os.path.join(directory, 'media')} has no picture — record one with scripts/site-demo "
            "and shot.mjs (CONTRIBUTING.md, Releasing)",
            file=sys.stderr,
        )
    if not args.out:
        print(post)
        return 0
    path = os.path.join(args.out, "post.md")
    if os.path.exists(path) and not args.force:
        print(f"compose.py: {path} exists — it may be hand-edited; pass --force to overwrite", file=sys.stderr)
        return 1
    os.makedirs(args.out, exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        f.write(post + "\n")
    print(path)
    return 0


if __name__ == "__main__":
    sys.exit(main())
