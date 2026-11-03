#!/usr/bin/env python3
"""두 moai 바이너리에 무작위 명령 수천 개를 똑같이 주고 바이트째 견준다 (moai-72qj.hjo).

    scripts/bindiff_fuzz.py --old A --new A                       옛 대 옛 — 도구를 믿기 전에 먼저 돌린다
    scripts/bindiff_fuzz.py --old A --new B --seed 7 --cases 3000
    scripts/bindiff_fuzz.py --old A --new B --seed 7 --case 412   그 한 사례만 다시

**왜 있나.** 사람이 고른 묶음(`bindiff.py`)은 사람이 떠올린 갈래만 본다. 여기서는 바이너리의 도움말에서
명령 나무(명령·플래그·위치 인자)를 읽고, 값 풀에서 무작위로 골라 사례를 짓는다. 격리·실행·비교는
`bindiff.py` 의 `Template`·`Sandbox`·`compare` 를 그대로 쓴다 — 가리개를 넓히지 않는다.

**사례는 시드와 번호 하나로 정해진다.** 사례 K 는 `Random(f"{시드}:{K}")` 만으로 짓는다. 입력 저장소의 id 는
꾸밈 때 새로 지어 판마다 다르니(`src/id.rs`), 사례 안에서는 `{id:m3}` 같은 이름으로 적어 두고 돌리기
직전에 푼다. 그래서 같은 시드·같은 번호는 늘 같은 사례다. 명령 나무가 같을 때(같은 `--old`)에 한해서.

**안전.** 값 풀의 경로는 `{repo}`·`{home}`·`{tmp}` 아래이거나 없는 경로뿐이다 — 실제 경로는 하나도 없다.
`..` 도 쓰지 않는다(자리의 바깥으로 나간다). 다만 `-C {home}` 처럼 `.moai` 가 없는 자리에서 친 moai 는 위로 올라가
트래커를 잡으니, `--work` 위에 트래커가 서 있으면 시작하지 않는다(`bindiff.tracker_above`).
빼는 것: `update`(망), `web`(`--dry-run` 만), `tui`(`--json` 만).

종료 코드: 차이나 흔들림(다시 돌리니 사라진 차이)이 있으면 1, 없으면 0, 준비가 안 되면 2.
"""

from __future__ import annotations

import argparse
import concurrent.futures as futures
import dataclasses
import json
import os
import random
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bindiff  # noqa: E402
from bindiff import Case, Sandbox, Template, compare, known_ids  # noqa: E402

TIMEOUT = 10.0


def say(*parts: object) -> None:
    print(*parts, file=sys.stderr, flush=True)


# ── 도움말에서 명령 나무 ──────────────────────────────────────────────


@dataclasses.dataclass
class Flag:
    long: str  # "--body"
    short: str | None
    value: str | None  # "<text>" 의 안쪽. 값이 없으면 None
    desc: str
    refs: list[str]  # 설명이 backtick 으로 가리키는 다른 플래그

    @property
    def takes_stdin(self) -> bool:
        d = self.desc.lower()
        return self.value is not None and ("stdin" in d or "`-` " in d or "`-` reads" in d)


@dataclasses.dataclass
class Positional:
    name: str
    required: bool
    variadic: bool
    desc: str


@dataclasses.dataclass
class Node:
    path: list[str]
    flags: list[Flag]
    positionals: list[Positional]
    subs: list[str]
    text: str


# 값을 안 받아도 되는 플래그(`--wiki [<slug>]`)도 받는다 — `<…>` 만 받던 판은 그 플래그를 말없이 빼 한 번도 안 돌렸다.
OPT_RE = re.compile(r"^ {2,6}(?:(-\w), )?(--[\w-]+)(?: \[?<([^>]+)>\]?)?(?: {2,}(.*))?$")
REF_RE = re.compile(r"`(--[\w-]+)")


def parse_help(moai: Path, path: list[str]) -> Node:
    p = subprocess.run([str(moai), *path, "--help"], capture_output=True, text=True, env={"PATH": "/usr/bin:/bin"})
    text = p.stdout
    lines = text.splitlines()
    flags: list[Flag] = []
    subs: list[str] = []
    poss: list[Positional] = []
    section = ""
    cur: Flag | None = None
    for line in lines:
        if line and not line.startswith(" "):
            section = line.rstrip()
            cur = None
            continue
        if section.startswith("Commands:"):
            if line.startswith("  ") and line.strip():
                name = line.split()[0]
                if name != "help":
                    subs.append(name)
            continue
        if section.startswith("Arguments:"):
            continue
        m = OPT_RE.match(line)
        if m and (section.startswith("Options:") or section.endswith(":") and "Filters" in section or "Order" in section):
            cur = Flag(m.group(2), m.group(1), m.group(3), (m.group(4) or "").strip(), [])
            flags.append(cur)
            continue
        if cur is not None and line.strip():
            cur.desc += " " + line.strip()
        elif not line.strip():
            cur = None
    for f in flags:
        f.refs = [r for r in REF_RE.findall(f.desc) if r != f.long]
    # 위 첫 규칙은 "Options:" 아래만 읽는다. "Filters (…):" 같은 다른 제목 아래의 플래그는 위에서 걸렀다 —
    # 제목이 `:` 로 안 끝나는 것(`Filters  (comma = or,  repeated = and):` 은 끝난다)을 위해 한 번 더 훑는다.
    # 위치 인자는 Usage 줄 끝에서 읽는다.
    for line in lines:
        if line.startswith("Usage:"):
            toks = line.split()[2:]  # "Usage:" <bin>
            toks = toks[len(path):] if toks[: len(path)] == path else toks
            prev = ""
            for t in toks:
                before, prev = prev, t
                if t in ("[OPTIONS]", "<COMMAND>", "[COMMAND]", "[OPTIONS]...") or before.startswith("--"):
                    continue
                m2 = re.match(r"^([<\[])([^>\]]+)[>\]](\.\.\.)?$", t)
                if m2:
                    poss.append(Positional(m2.group(2), m2.group(1) == "<", bool(m2.group(3)), ""))
            break
    # 위치 인자의 설명
    in_args = False
    for line in lines:
        if line.startswith("Arguments:"):
            in_args = True
            continue
        if in_args:
            if line and not line.startswith(" "):
                break
            m3 = re.match(r"^ {2}[<\[]([^>\]]+)[>\]](?:\.\.\.)? *(.*)$", line)
            if m3:
                for q in poss:
                    if q.name == m3.group(1):
                        q.desc = m3.group(2)
            elif line.strip():
                for q in poss[-1:]:
                    q.desc += " " + line.strip()
    return Node(path, flags, poss, subs, text)


def build_tree(moai: Path) -> list[Node]:
    out: list[Node] = []

    def walk(path: list[str]) -> None:
        n = parse_help(moai, path)
        out.append(n)
        if len(path) < 3:
            for s in n.subs:
                walk(path + [s])

    walk([])
    return out


# ── 값 풀 ─────────────────────────────────────────────────────────────

# 입력 저장소의 이름 붙은 id (`Template.ids` 의 열쇠). 값은 판마다 달라 사례에는 `{id:열쇠}` 로 적는다.
ID_KEYS = [
    "milestone", "epic", "m1", "m2", "m3", "m4", "m5", "bug", "theirs", "unowned",
    "child", "late", "backlog", "backlog2", "removed", "done", "review", "archived",
]
ACTOR = bindiff.ACTOR
OTHER = bindiff.OTHER
LONG = "가나다 abc " * 600

TEXTS = [
    "새 일", "x", "", " ", "한국어 제목", "-x", "--help", "--", "line1\nline2", "tab\there", "끝에 공백  ",
    "ctrl\x01\x07\x1b[31mred", "quote ' \" `bt` $(x) ${H}", "emoji 😀 이모지", "# heading", "- [p1] x #t @2026-11-05",
    "@2026-11-05", "#tag", "model: anthropic/opus-5.5 (high — 쓰기 경로)", "Next: 다음 걸음", "{{v}}", "{repo}", "{tmp}/x",
    "파서가 BOM 에서 죽는다", "review — 첫 일", "a" * 300, LONG, "0", "-1", "moai-zzzz", "null", "\r\n", "탭\t과 줄\n바꿈",
]
BAD_IDS = ["moai-zzzz", "moai-", "MOAI-ABCD", "moai-12345", "x", "", "moai-aaaa.", "moai-aaaa.xyz", "moai--", "-moai-a", "moai-é한글", "0", "-1"]
COLUMNS = ["todo", "in_progress", "review", "done"]
BAD_COLUMNS = ["nowhere", "TODO", "in progress", "", "-", "done,todo", "backlog"]
KINDS = ["issue", "epic", "milestone", "backlog", "bug", "chore", "review", "feature", "enhancement", "", "x"]
TAGS = ["parser", "ui", "bug", "new", "a,b", "한국어", "", "-x", "A B", "x" * 80, "parser,ui"]
PEOPLE = [ACTOR, OTHER, "none", "me", "Kim", "kim@example.com", "", "Name (bad", "Diff Test", "other@example.com", "한 글 (han@example.com)"]
DATES = [
    "2026-11-05", "2026-11-03", "2026-11-04", "2026-12-31", "2026-10-01", "2026-02-30", "2026-13-01", "2025-01-01", "+3", "+0", "+1",
    "-3", "+999", "none", "today", "", "x", "11-05", "2026-11-05T10:00:00Z", "3d", "1w", "7", "2026-11-01..2026-11-30",
    "2026-11-01~2026-11-30", "2026-11-01~", "..2026-11-30", "2026-11-30..2026-11-01", "2026-11-05..", "yesterday", "2026-1-5",
]
NUMBERS = ["0", "1", "2", "3", "4", "5", "10", "-1", "-2", "100", "999999999999999999999", "abc", "", "1.5", "+2", "0x10"]
PRIORITIES = ["0", "1", "2", "3", "4", "-1", "x", "", "p1", "9"]
SORTS = [
    "priority", "updated:desc", "due", "status:asc,title", "created", "id", "assignee", "title:desc", "priority,updated:desc",
    "due:desc", "nope", "", "priority,priority", "status:up", "created,id,title", ",",
]
FILTERS = [
    "status=todo", "status=done,todo", "due_on=2026-11-01~2026-11-30", "starts_on=2026-11-01~", "tag=parser", "nope=1", "status", "=",
    "priority=0", "kind=epic", "assignee=me", "", "status=todo status=done", "due_on=x",
]
FIELDS = ["id,title,status", "id", "", "nope", "id,id", "title,body,notes", "status,priority,updated"]
WORDS = ["worktree", "칸", "status", "", "the", "moai show", "x" * 50, "--", "-v", "ready", "milestone", "'quote", "\"dq\"", "*", "a b c"]
# 위키 쪽(`docs/` 의 페이지 이름)과 없는 것·틀린 것. `wiki show <slug>` 가 늘 "없는 쪽" 만 돌지 않게.
SLUGS = ["cli", "workflow", "agents", "ctl", "explorer", "glossary", "json", "nope", "", "CLI", "../cli", "cli.md"]
# `add --from … --var <name=value>`. 계획의 `{{v}}` 를 채우는 갈래.
VARS = ["v=1.2", "version=2.0", "v=", "=x", "x", "", "v=1=2", "한=값"]
PATHS_DIR = ["{repo}", "{repo}/src", "{repo}/docs", "{repo}/.moai", "{home}", "{tmp}", "{tmp}/nope", "/nonexistent-moai-fuzz/x", "", ".", "src", "docs", "{repo}/Cargo.toml"]
PATHS_FILE = ["{tmp}/dot.png", "{tmp}/note.txt", "{tmp}/plan.md", "{tmp}/missing", "/nonexistent-moai-fuzz/f", "", "{repo}/Cargo.toml", "{repo}/.moai/issues.jsonl", "{tmp}", "{repo}/src"]
CHOICES = {
    "color": ["auto", "always", "never", "sometimes", ""],
    "dialect": ["claude", "codex", "antigravity", "other", ""],
    "guide": ["block", "file", "hook", "none", "x"],
    "tracking": ["commit", "exclude", "gitignore", "x"],
    "scope": ["local", "user", "project", "x", ""],
    "agent": ["claude", "codex", "antigravity", "all", "x"],
    "skill": ["moai-tmux", "moai-cmux", "moai-saycode", "moai-wiki", "moai-supervise", "x", ""],
    "bucket": ["day", "week", "month"],
    "by": ["status", "kind", "tag", "assignee", "epic", "priority", "model", "x", "status,tag"],
    "in": ["title", "body", "id", "tag", "all", "x"],
    "colour": ["red", "#ff0000", "blue", "none", "x", ""],
}
HOOK_EVENTS = ["session-start", "user-prompt-submit", "pre-tool-use", "stop", "pre-invocation", "nope", ""]
CTL_WORDS = ["list", "board", "gantt", "stats", "wiki", "column:todo", "priority", "title", "UTC", "Asia/Seoul", "dark", "light", "x", "", "show", "hide", "left", "right", "bottom"]
HASHES = ["abc123", "0" * 16, "", "x"]
# `merge-driver` 의 자리 인자와 `merge_files` 가 까는 파일 이름(git 의 `%O %A %B`).
MERGE_SLOTS = {"base": "O", "ours": "A", "theirs": "B"}
GLOBAL_FLAGS = {"--json", "--no-color", "--color", "--dir", "--user", "--help", "--version"}


def pick_id(r: random.Random, allow_none: bool, hist: list[str]) -> str:
    x = r.random()
    if x < 0.62:
        return "{id:%s}" % r.choice(ID_KEYS)
    if x < 0.74 and hist:
        return r.choice(hist)
    if x < 0.80 and allow_none:
        return "none"
    return r.choice(BAD_IDS)


def kind_of(name: str, hint: str | None, desc: str) -> str:
    n = name.lower().lstrip("-")
    h = (hint or "").lower()
    if n in ("epic", "milestone", "parent", "after", "issue", "blocks", "unblocks", "to", "drop", "id", "target_id") or n.startswith("id"):
        return "id+" if "none" in h or n in ("epic", "milestone", "parent") else "id"
    if n == "target":
        return "target"
    if n == "path" and "id" in h:  # `tui --path <id|basket>`
        return "id"
    if n in ("grep", "words", "query"):
        return "words"
    if n == "var":
        return "var"
    # **칸보다 먼저 본다** — `add --from <file|->`·`backlog promote --from <file|->` 의 `--from` 은 계획이다. 칸 규칙 뒤에
    # 두던 판은 그 둘에 칸 이름만 주어 계획 갈래(`{tmp}/plan.md`·stdin 의 계획)가 거의 안 돌았다. `mv`·`defer` 의
    # `--from <column>` 은 그대로 칸이다.
    if "from" in n and "file" in h:
        return "plan"
    if n in ("status", "from", "column"):
        return "column"
    if n == "priority":
        return "priority"
    if n == "type":
        return "type"
    if n in ("tag", "no-tag", "untag"):
        return "tag"
    if n in ("assignee", "user"):
        return "person"
    if n in ("start", "due", "since", "created", "done"):
        return "date"
    if n in ("shift", "shift-start", "shift-due"):
        return "shift"
    if n in ("limit", "stale", "last", "port", "line", "wait", "ttl", "list-width", "list-height", "wiki-width", "gantt-label", "gantt-grid", "marker", "n"):
        return "number"
    if n == "sort":
        return "sort"
    if n == "filter":
        return "filter"
    if n == "fields":
        return "fields"
    if n in ("dir", "project") or h == "path":
        return "dir"
    if n in ("file", "base", "ours", "theirs", "path"):
        return "file"
    if n == "event":
        return "event"
    if n == "match":
        return "hash"
    if n == "host":
        return "host"
    if n == "as" or n == "then":
        return "text"
    if n in CHOICES:
        return "choice:" + n
    if h in CHOICES:  # `--with <skill>`·`--without <skill>` 처럼 이름 대신 값 꼴이 고르는 것
        return "choice:" + h
    if "|" in h and "id" not in h and "date" not in h and "who" not in h:
        return "pipe"
    return "text"


def pick_value(r: random.Random, kind: str, hint: str | None, hist: list[str]) -> str:
    x = r.random()
    if kind in ("id", "id+"):
        return pick_id(r, kind == "id+", hist)
    if kind == "column":
        return r.choice(COLUMNS) if x < 0.8 else r.choice(BAD_COLUMNS)
    if kind == "priority":
        return r.choice(PRIORITIES[:4]) if x < 0.75 else r.choice(PRIORITIES)
    if kind == "type":
        return r.choice(KINDS[:4]) if x < 0.7 else r.choice(KINDS)
    if kind == "tag":
        return r.choice(TAGS)
    if kind == "person":
        return r.choice(PEOPLE)
    if kind == "date":
        return r.choice(DATES)
    if kind == "shift":
        return r.choice(["1", "2", "-1", "-3", "0", "+2", "x", "", "999", "1.5"])
    if kind == "number":
        return r.choice(NUMBERS[:10]) if x < 0.8 else r.choice(NUMBERS)
    if kind == "sort":
        return r.choice(SORTS)
    if kind == "filter":
        return r.choice(FILTERS)
    if kind == "fields":
        return r.choice(FIELDS)
    if kind == "dir":
        return r.choice(PATHS_DIR)
    if kind == "file":
        return r.choice(PATHS_FILE)
    if kind == "event":
        return r.choice(HOOK_EVENTS)
    if kind == "hash":
        return r.choice(HASHES)
    if kind == "words":
        return r.choice(WORDS) if x < 0.85 else r.choice(TEXTS)
    if kind == "var":
        return r.choice(VARS)
    if kind == "slug":
        return r.choice(SLUGS)
    if kind == "host":
        return r.choice(["127.0.0.1", "localhost", "0.0.0.0", "x", "", "::1"])
    if kind == "target":
        return r.choice(["1", "0", "{id:epic}", "x", "", "all", "tui", "web"])
    if kind.startswith("choice:"):
        return r.choice(CHOICES[kind[7:]])
    if kind == "pipe" and hint:
        opts = hint.split("|")
        return r.choice(opts) if x < 0.85 else r.choice(TEXTS)
    if kind == "plan":
        return "-" if x < 0.55 else r.choice(["{tmp}/plan.md", "{tmp}/missing", "", "{tmp}/note.txt"])
    return r.choice(TEXTS)


# ── stdin 글 ──────────────────────────────────────────────────────────


def plan_text(r: random.Random) -> str:
    """`add --from` 마크다운. 대개 맞는 꼴이고 가끔 어긋난다."""
    tags = ["#parser", "#ui", "#bug", "#enhancement", "#한국어", ""]
    dates = ["@2026-11-10", "@2026-11-08", "@2026-11-25", "@2026-10-01", "@2026-11-05~2026-11-12", "@2026-11-05~", "@2026-02-30", ""]
    lines = []
    if r.random() < 0.9:
        lines.append("# " + r.choice(["새 에픽", "펼친 에픽 {{v}}", "Epic", "x" * 60, "릴리스 {{v}}"]) + " " + r.choice(dates))
    for _ in range(r.choice([0, 1, 2, 3, 5, 8])):
        p = r.choice(["[p0]", "[p1]", "[p2]", "[p3]", "[p4]", "", "[pX]", "[p1]"])
        title = r.choice(["하나", "둘", "{{v}} 를 낸다", "title", "긴 " * 20, "", "a #b #c"])
        lines.append(f"- {p} {title} {r.choice(tags)} {r.choice(dates)}".rstrip())
    if r.random() < 0.15:
        lines.insert(r.randrange(len(lines) + 1), r.choice(["  - [p1] 들여쓴 것", "## 소제목", "본문 줄", "", "---", "* [p1] 별표"]))
    nl = r.choice(["\n", "\n", "\n", "\r\n"])
    return nl.join(lines) + r.choice(["", nl])


def free_text(r: random.Random) -> str:
    x = r.random()
    if x < 0.25:
        return plan_text(r)
    if x < 0.4:
        return ""
    if x < 0.55:
        return "## 새 본문\n\n- 하나\n- 둘\n"
    return "\n".join(r.choice(TEXTS) for _ in range(r.choice([1, 2, 3, 6]))) + r.choice(["", "\n"])


def rows_text(r: random.Random) -> list[str]:
    """merge-driver 용 한 줄 한 이슈."""
    return [bindiff.merge_row("moai-aaa" + "abcd"[k], t) for k, t in enumerate(["하나", "둘", "셋", "넷"])]


def merge_files(r: random.Random) -> dict[str, bytes]:
    base = rows_text(r)

    def mut(ls: list[str]) -> list[str]:
        out = list(ls)
        for _ in range(r.choice([0, 1, 1, 2])):
            k = r.randrange(len(out)) if out else 0
            if not out:
                break
            op = r.choice(["title", "status", "tags", "drop", "garbage", "dup", "empty"])
            if op == "title":
                out[k] = out[k].replace('"title":"', '"title":"' + r.choice(["!", "ours-", "theirs-"]))
            elif op == "status":
                out[k] = out[k].replace('"todo"', '"' + r.choice(["done", "review", "in_progress"]) + '"')
            elif op == "tags":
                out[k] = out[k].replace('"status"', '"tags":["%s"],"status"' % r.choice(["a", "b", "parser"]))
            elif op == "drop":
                del out[k]
            elif op == "garbage":
                out.append(r.choice(["not json", '{"id":"moai-brok","title":', "", "\xef\xbf\xbd"]))
            elif op == "dup":
                out.append(out[k])
            elif op == "empty":
                out = []
        return out

    files = {}
    for name, ls in (("O", base), ("A", mut(base)), ("B", mut(base))):
        files[name] = ("\n".join(ls) + r.choice(["\n", "\n", ""])).encode()
    return files


# ── 사례 짓기 ─────────────────────────────────────────────────────────


@dataclasses.dataclass
class Gen:
    nodes: list[Node]
    paths: list[Node]  # 고를 수 있는 명령(루트 포함, update 뺌)
    dep_cases: list[tuple[Node, Flag]]
    hist_ids: list[str]
    help_off: int
    dep_off: int
    hook_samples: dict[str, list[dict]]


def usable(n: Node) -> bool:
    return not (n.path and n.path[0] == "update")


def build_gen(old: Path, template: Template | None) -> Gen:
    nodes = [n for n in build_tree(old) if usable(n)]
    dep = []
    pat = re.compile(r"(?i)\b(with|without|requires?|only|means nothing|alone|needs?)\b")
    for n in nodes:
        for f in n.flags:
            if f.long in GLOBAL_FLAGS:
                continue
            if f.refs or pat.search(f.desc):
                dep.append((n, f))
    hist: list[str] = []
    if template is not None:
        hist = history_ids(template)
    samples: dict[str, list[dict]] = {"codex": [], "antigravity": []}
    for d in samples:
        for f in sorted((bindiff.REPO / "tests" / "hooks" / d).glob("*.json")):
            try:
                samples[d].append({"stem": f.stem, "data": bindiff.swap_paths(json.loads(f.read_text()))})
            except (OSError, ValueError):
                pass
    return Gen(nodes, nodes, dep, hist, 0, 0, samples)


def history_ids(template: Template) -> list[str]:
    """꾸미기 전 커밋에 있던 줄의 id. 커밋이 같으면 늘 같다."""
    ids: set[str] = set()
    names = bindiff.git(template.path, "ls-tree", "-r", "--name-only", template.commit, "--", ".moai").split()
    for nm in names:
        if nm.endswith(".jsonl") and "/journal/" not in nm:
            out = bindiff.git(template.path, "show", f"{template.commit}:{nm}", check=False)
            ids.update(m.decode() for m in bindiff.ID_RE.findall(out.encode()))
    return sorted(ids)[:400]


def weights(nodes: list[Node]) -> list[float]:
    w = []
    for n in nodes:
        k = len([f for f in n.flags if f.long not in GLOBAL_FLAGS])
        leaf = not n.subs
        w.append((1.0 + k ** 0.5) * (1.0 if leaf else 0.25))
    return w


def render_flag(r: random.Random, f: Flag, val: str | None) -> list[str]:
    name = f.long
    if f.short and r.random() < 0.15:
        name = f.short
    if val is None:
        return [name]
    if r.random() < 0.15 and name.startswith("--"):
        return [f"{name}={val}"]
    return [name, val]


def flag_value(r: random.Random, node: Node, f: Flag, hist: list[str]) -> str | None:
    if f.value is None:
        return None
    kind = kind_of(f.long, f.value, f.desc)
    if f.takes_stdin and kind in ("text", "pipe") and r.random() < 0.4:
        return "-"
    return pick_value(r, kind, f.value, hist)


def make_case(g: Gen, seed: str, k: int) -> Case:
    r = random.Random(f"{seed}:{k}")
    node: Node
    dep_flag: Flag | None = None
    n_help = len(g.paths)
    mode = "random"
    if k % 12 == 0:
        mode = "help"
    elif k % 12 == 6 and g.dep_cases:
        mode = "dep"
    elif r.random() < 0.03:
        mode = "garbage"
    elif r.random() < 0.01:
        mode = "helpcmd"

    env: dict[str, str | None] = {}
    stdin: bytes | None = None
    pre: list[str] = []
    files: dict[str, bytes] = {}

    if mode == "help":
        node = g.paths[(k // 12 + g.help_off) % n_help]
        form = r.choice(["--help", "--help", "-h", "help"])
        if form == "help" and node.path:
            args = ["help", *node.path]
        else:
            args = [*node.path, "--help" if form == "help" else form]
        if r.random() < 0.15:
            args.append(r.choice(["--json", "--color", "always"] if r.random() < 0.5 else ["--no-color"]))
        return Case(str(k), args, timeout=TIMEOUT)
    if mode == "helpcmd":
        return Case(str(k), r.choice([["help"], ["help", "nope"], ["--version"], ["-V"], [], ["help", "help"], ["--help", "--json"]]), timeout=TIMEOUT)
    if mode == "garbage":
        junk = r.choice(TEXTS + ["frobnicate", "--frob", "-Z", "show--tree", "STATUS", "stat"])
        args = r.choice([[junk], [junk, "--json"], ["status", junk], ["--frob", "status"], [junk, junk]])
        return Case(str(k), args, timeout=TIMEOUT)

    if mode == "dep":
        node, dep_flag = g.dep_cases[(k // 12 + g.dep_off) % len(g.dep_cases)]
    else:
        node = r.choices(g.paths, weights(g.paths))[0]

    path = list(node.path)
    hist = g.hist_ids

    # 고를 플래그
    local = [f for f in node.flags if f.long not in GLOBAL_FLAGS]
    chosen: list[Flag] = []
    if dep_flag is not None:
        chosen.append(dep_flag)
        if r.random() < 0.4:
            for ref in dep_flag.refs:
                f2 = next((f for f in local if f.long == ref), None)
                if f2 is not None and r.random() < 0.7:
                    chosen.append(f2)
        extra = r.choice([0, 0, 1, 2])
    else:
        extra = r.choice([0, 1, 1, 2, 2, 3, 4])
    pool = [f for f in local if f not in chosen]
    r.shuffle(pool)
    for f in pool[:extra]:
        chosen.append(f)
    # 고른 것이 가리키는 플래그를 곁들이거나 일부러 뺀다
    names = {f.long for f in chosen}
    for f in list(chosen):
        for ref in f.refs:
            f2 = next((x for x in local if x.long == ref), None)
            if f2 is None:
                continue
            if f2.long in names and dep_flag is None and r.random() < 0.3 and f2 is not dep_flag:
                chosen.remove(f2)
                names.discard(f2.long)
            elif f2.long not in names and r.random() < 0.25 and dep_flag is None:
                chosen.append(f2)
                names.add(f2.long)
    if dep_flag is not None:
        # 가리키는 플래그를 일부러 뺀다(빼는 쪽이 오류 갈래를 연다)
        if r.random() < 0.7:
            chosen = [f for f in chosen if f is dep_flag or f.long not in dep_flag.refs]

    # 강제: tui 는 --json, web 은 --dry-run
    forced: list[str] = []
    if path[:1] == ["tui"]:
        forced = ["--json"]
    if path[:1] == ["web"]:
        forced = ["--dry-run"]
        chosen = [f for f in chosen if f.long != "--dry-run"]

    units: list[list[str]] = []
    stdin_text = False
    plan_in = False
    for f in chosen:
        for _ in range(2 if r.random() < 0.06 and f.value is not None else 1):
            v = flag_value(r, node, f, hist)
            if v == "-":
                stdin_text = True
                plan_in = plan_in or kind_of(f.long, f.value, f.desc) == "plan"
            units.append(render_flag(r, f, v))

    # 위치 인자
    pos_units: list[list[str]] = []
    drop_all = r.random() < 0.06
    for p in node.positionals:
        if drop_all:
            break
        # 머지 드라이버의 세 파일은 사례 앞에 `{tmp}/O·A·B` 로 깐다(`merge_files`) — 셋이 다 서야 이슈별 3-way 가 돈다.
        merging = path[:1] == ["merge-driver"] and p.name in MERGE_SLOTS
        if not p.required and r.random() < (0.1 if merging else 0.45):
            continue
        if p.required and r.random() < 0.05:
            continue
        kind = kind_of(p.name, None, p.desc)
        if p.name == "target" and path[-1:] == ["show"]:
            kind = "id"
        if p.name in ("file", "base", "ours", "theirs") or p.name.startswith("file"):
            kind = "file"
        if p.name.lower() in ("text", "title", "lesson", "keys", "prefix", "name", "value"):
            kind = "text"
        if p.name.lower() in ("words", "query"):
            kind = "words"
        if p.name.lower() == "slug":
            kind = "slug"
        if p.name.lower() in ("name", "value") and path[:1] == ["ctl"]:
            kind = "ctl"
        n = r.choice([1, 1, 1, 2, 3]) if p.variadic else 1
        if r.random() < 0.04:
            n += 1
        vals = []
        for _ in range(n):
            if kind == "ctl":
                vals.append(r.choice(CTL_WORDS))
            elif p.name == "colour":
                vals.append(r.choice(CHOICES["colour"]))
            elif p.name == "marker":
                vals.append(r.choice(["7", "5", "x", ""]))
            elif merging:
                vals.append("{tmp}/" + MERGE_SLOTS[p.name] if r.random() < 0.85 else r.choice(PATHS_FILE))
            elif p.name == "path" and path[:1] == ["merge-driver"]:
                vals.append(r.choice([".moai/issues.jsonl", "x", "", "{repo}/.moai/issues.jsonl"]))
            elif kind == "text" and r.random() < 0.15 and p.name.lower() in ("text", "title"):
                vals.append(pick_id(r, False, hist))
            else:
                vals.append(pick_value(r, kind, None, hist))
        if path[-1:] in (["mv"], ["defer"]) and p.variadic and path[-1:] == ["mv"]:
            vals.append(r.choice(COLUMNS) if r.random() < 0.85 else r.choice(BAD_COLUMNS))
        pos_units.append(vals)
    # 합치기
    flat_pos = [v for vs in pos_units for v in vs]
    order = r.random()
    body: list[str] = []
    fl = [t for u in units for t in u]
    if order < 0.65:
        body = flat_pos + fl
    elif order < 0.85:
        body = fl + flat_pos
    else:
        allu = [[v] for v in flat_pos] + units
        r.shuffle(allu)
        body = [t for u in allu for t in u]
    if any(v.startswith("-") for v in flat_pos) and r.random() < 0.5:
        # 위치 인자가 `-` 로 시작하면 `--` 로 막기도 한다
        body = fl + ["--"] + flat_pos

    if path[:1] == ["hook"]:
        dialect = r.choice(["claude", "claude", "codex", "antigravity", None])
        sample = None
        if dialect in g.hook_samples and g.hook_samples[dialect] and r.random() < 0.7:
            sample = r.choice(g.hook_samples[dialect])
        event = bindiff.hook_event_of(sample["stem"]) if sample else r.choice(HOOK_EVENTS[:5] + HOOK_EVENTS[:4])
        if r.random() < 0.04:
            event = r.choice(HOOK_EVENTS)
        body = [event] + (["--dialect", dialect] if dialect else [])
        if r.random() < 0.05:
            body += ["--dialect", r.choice(CHOICES["dialect"])]
        stdin = hook_stdin(r, sample, event)
        if r.random() < 0.5:
            env["CLAUDE_CODE_SESSION_ID"] = "s1"
        stdin_text = False

    # 전역 장식
    glob: list[str] = []
    x = r.random()
    if x < 0.28 and "--json" not in forced:
        glob.append("--json")
    if r.random() < 0.09:
        glob += ["--color", r.choice(["always", "always", "never", "auto", "sometimes"])]
    elif r.random() < 0.03:
        glob.append("--no-color")
    if r.random() < 0.04:
        glob += ["-C" if r.random() < 0.5 else "--dir", r.choice(PATHS_DIR)]
    if r.random() < 0.03:
        glob += ["--user", r.choice(PEOPLE)]
    if r.random() < 0.02 and path:
        glob += [r.choice(["--frobnicate", "--json=x", "-Q"])]
    head = r.random() < 0.3
    args = (glob + path + forced + body) if head else (path + forced + body + glob)

    if stdin is None:
        if plan_in and r.random() < 0.8:
            stdin = plan_text(r).encode()
        elif "-" in args or stdin_text:
            stdin = free_text(r).encode()
        elif r.random() < 0.08:
            stdin = free_text(r).encode()

    # 환경
    if r.random() < 0.06:
        env["MOAI_ACTOR"] = r.choice([None, OTHER, "", "Solo", ACTOR])
    if r.random() < 0.02:
        env["MOAI_HERE"] = "1"
    if r.random() < 0.02:
        env["NO_COLOR"] = "1"
    if r.random() < 0.015:
        env["MOAI_NOW"] = r.choice(["2026-11-04T03:00:00Z", "2027-01-01T00:00:00Z", "garbage"])

    # 사전 상태
    if r.random() < 0.08:
        pre.append(r.choice(["broken", "unknown_field", "held", "held", "dirty_gitignore"]))
        if "held" in pre:
            env["CLAUDE_CODE_SESSION_ID"] = "s1"

    # 자리 아래 깔 파일 — {tmp} 를 부르는 사례만
    blob = "\0".join(args) + "\0" + (stdin.decode(errors="replace") if stdin else "")
    if "{tmp}" in blob or path[:1] == ["merge-driver"]:
        files.update(merge_files(r))
        files["dot.png"] = bindiff.tiny_png() if r.random() < 0.85 else b"not a png"
        files["note.txt"] = "메모\n".encode()
        files["plan.md"] = plan_text(r).encode()

    before = make_before(files, pre)
    # 안전: NUL 이 든 글은 argv 로 못 간다
    args = [a.replace("\0", "") for a in args]
    return Case(str(k), args, stdin=stdin, env=env, before=before, timeout=TIMEOUT)


def hook_stdin(r: random.Random, sample: dict | None, event: str) -> bytes | None:
    if sample is not None and r.random() < 0.85:
        return json.dumps(sample["data"], ensure_ascii=False).encode()
    x = r.random()
    if x < 0.05:
        return b""
    if x < 0.12:
        return r.choice([b"{not json", b"[]", b"null", b"{}", b'{"session_id":1}', "한글".encode()])
    tools = {
        "Read": lambda: {"file_path": r.choice(["{repo}/src/main.rs", "{repo}/Cargo.toml", "/nonexistent-moai-fuzz/x", "{tmp}/x"])},
        "Edit": lambda: {"file_path": r.choice(["{repo}/src/main.rs", "{repo}/.moai/issues.jsonl", "{tmp}/x", "{repo}/.worktrees/moai-x/a"]), "old_string": "a", "new_string": "b"},
        "Write": lambda: {"file_path": r.choice(["{repo}/src/new.rs", "{tmp}/x", "{repo}/docs/a.md", "{home}/.config/x"]), "content": "x"},
        "MultiEdit": lambda: {"file_path": "{repo}/src/main.rs", "edits": []},
        "Bash": lambda: {"command": r.choice(BASH_COMMANDS)},
        "Grep": lambda: {"pattern": "x"},
        "Task": lambda: {"prompt": "x"},
    }
    tool = r.choice(list(tools))
    body = {
        "session_id": r.choice(["s1", "s1", "s2", ""]),
        "cwd": r.choice(["{repo}", "{repo}", "{repo}/src", "{home}", "{tmp}", "/nonexistent-moai-fuzz"]),
        "hook_event_name": r.choice(["PreToolUse", "SessionStart", "Stop", "UserPromptSubmit"]),
        "tool_name": tool,
        "tool_input": tools[tool](),
        "prompt": r.choice(TEXTS),
        "source": r.choice(["startup", "compact", "resume", "clear"]),
    }
    for key in r.sample(list(body), r.choice([0, 0, 1, 2])):
        if key not in ("cwd",):
            del body[key]
    return json.dumps(body, ensure_ascii=False).encode()


BASH_COMMANDS = [
    "moai add '딴 일'", "moai mv {id:theirs} in_progress", "moai mv {id:theirs} in_progress --take -m ok",
    "moai mv {id:unowned} review", "moai add 'review — x' -t review --parent {id:m1}",
    "moai add 'review — x' -t review --parent {id:m1} -b 관점", "tmux kill-server", "tmux -L t kill-server",
    "env -u TMUX tmux -L t kill-session", "pkill tmux", "sed -i s/a/b/ src/lib.rs", "echo x > src/a.txt",
    "echo x >> .moai/x", "cp a src/b", "git status", "ls", "moai backlog add 'x'", "moai add 'x' -e {id:epic}",
    "moai mv {id:m3} in_progress", "moai mv {id:m3} in_progress --from todo", "cd /tmp && moai mv {id:m3} in_progress",
    "moai note {id:m3} 'x'", "cat <<EOF > src/z.rs\nx\nEOF", "tee src/q < /dev/null", "moai mv {id:m3} done -m ok",
    "moai defer {id:m3} -m x", "install -m 644 a src/b", "rm -rf src", "moai epic add 'x'", "moai milestone add 'v1'",
]


# ── 사전 상태 ─────────────────────────────────────────────────────────

IDS_NOW: dict[str, str] = {}


def make_before(files: dict[str, bytes], pre: list[str]):
    if not files and not pre:
        return None

    def before(sb: Sandbox) -> None:
        for name, data in files.items():
            (sb.tmp / name).write_bytes(data)
        for step in pre:
            if step == "broken":
                f = sb.repo / ".moai" / "issues.jsonl"
                f.write_bytes(f.read_bytes() + b'{"id":"moai-brok","title":\n')
            elif step == "unknown_field":
                f = sb.repo / ".moai" / "issues.jsonl"
                key = IDS_NOW.get("m3", "").encode()
                lines = f.read_bytes().split(b"\n")
                for n, line in enumerate(lines):
                    if key and line.startswith(b'{"id":"' + key + b'"'):
                        lines[n] = line[:-1] + b',"zz_future":{"a":[1,2]}}'
                f.write_bytes(b"\n".join(lines))
            elif step == "held":
                env = sb.env("en", {"CLAUDE_CODE_SESSION_ID": "s1"})
                subprocess.run([bindiff.HOOK_BIN[0], "mv", IDS_NOW["m3"], "in_progress"], cwd=sb.repo, env=env, capture_output=True)
            elif step == "dirty_gitignore":
                (sb.repo / ".gitignore").write_text("*.tmp\n")
                (sb.repo / "stray.tmp").write_text("x\n")

    return before


def resolve_text(s: str) -> str:
    return re.sub(r"\{id:(\w+)\}", lambda m: IDS_NOW.get(m.group(1), m.group(0)), s)


def resolve_case(c: Case) -> Case:
    # `dataclasses.replace` 다 — 칸을 손으로 옮기던 판은 `cwd` 를 떨궜다. 나중에 `Case` 에 칸이 늘어도 따라온다.
    return dataclasses.replace(
        c,
        args=[resolve_text(a) for a in c.args],
        stdin=resolve_text(c.stdin.decode()).encode() if c.stdin is not None else None,
        env={k: (resolve_text(v) if v is not None else None) for k, v in c.env.items()},
    )


# ── 돌림 ──────────────────────────────────────────────────────────────


def describe(c: Case) -> list[str]:
    out = ["    moai " + shlex.join(c.args)]
    if c.env:
        out.append("    env " + " ".join(f"{k}={'<unset>' if v is None else shlex.quote(v)}" for k, v in c.env.items()))
    if c.stdin is not None:
        t = c.stdin.decode(errors="replace")
        out.append(f"    stdin ({len(c.stdin)}바이트) {t[:300]!r}" + (" …" if len(t) > 300 else ""))
    if c.before is not None:
        out.append("    (사전 상태·파일은 시드로 다시 지어진다)")
    return out


def fuzz(args: argparse.Namespace, old: Path, new: Path, work: Path, template: Template, gen: Gen) -> int:
    known = known_ids(template)
    IDS_NOW.clear()
    IDS_NOW.update(template.ids)
    jobs = max(1, args.jobs)
    boxes = [Sandbox(work / f"box{n}", template) for n in range(jobs)]
    say(f"자리 {jobs}개를 짓는다")
    with futures.ThreadPoolExecutor(jobs) as ex:
        list(ex.map(lambda b: b.build(), boxes))
    todo = [args.case] if args.case is not None else list(range(args.cases))
    queue = list(todo)
    lock = threading.Lock()
    found: list[tuple[int, str, Case, list[str]]] = []
    shaky: list[tuple[int, str, Case, list[str]]] = []
    harness: list[tuple[int, str, str]] = []
    codes: dict[str, int] = {}
    done = [0]
    total = len(todo) * len(args.lang)
    started = time.time()
    seed = args.seed

    def worker(box: Sandbox) -> None:
        while True:
            with lock:
                if not queue:
                    return
                k = queue.pop(0)
            try:
                case = resolve_case(make_case(gen, str(seed), k))
            except Exception as e:  # 생성기의 버그는 조용히 넘기지 않는다
                with lock:
                    harness.append((k, "-", f"생성 실패: {e!r}"))
                continue
            for lang in args.lang:
                try:
                    a = box.run(old, case, lang)
                    b = box.run(new, case, lang)
                    # 가리개는 `bindiff.compare` 하나다 — 퍼저 쪽에서 따로 넓히지 않는다.
                    d = compare(a, b, known | bindiff.case_ids(case))
                    flaky = False
                    if d:
                        # 한 번 더 돌려 가른다. 다시 돌려도 나오면 두 판의 차이다. 사라지면 "흔들림" 이다 — 가리개가
                        # 아는 흔들림(새 id·집기 나노초)은 bindiff 가 이미 다루니, 남은 흔들림은 제품의 비결정이거나
                        # 이 도구의 틈이다. 어느 쪽이든 관문을 지나게 두지 않는다(실패로 센다).
                        a2 = box.run(old, case, lang)
                        b2 = box.run(new, case, lang)
                        d2 = compare(a2, b2, known | bindiff.case_ids(case))
                        if not d2:
                            flaky = True
                        else:
                            d = d2
                except (Exception, SystemExit) as e:
                    # `bindiff.git` 은 `SystemExit` 로 진다 — `Exception` 만 받던 판은 한 자리의 git 하나가 진 것으로 일꾼이
                    # 통째로 죽고, 남은 일꾼이 줄을 다 비운 뒤에야 그 하나만 찍혀 찾은 차이의 목록이 사라졌다.
                    with lock:
                        harness.append((k, lang, f"실행 실패: {e!r}"))
                    continue
                with lock:
                    done[0] += 1
                    codes[str(a.code)] = codes.get(str(a.code), 0) + 1
                    if d and flaky:
                        shaky.append((k, lang, case, d))
                        say(f"흔들림(다시 돌리니 사라짐): 사례 {k} [{lang}]")
                    elif d:
                        found.append((k, lang, case, d))
                        say(f"차이: 사례 {k} [{lang}] moai {shlex.join(case.args)[:120]}")
                    if done[0] % 500 == 0:
                        say(f"  {done[0]}/{total}  {time.time() - started:.0f}초")

    with futures.ThreadPoolExecutor(jobs) as ex:
        list(ex.map(worker, boxes))
    took = time.time() - started

    print(f"시드 {seed}, 사례 {len(todo)}개 × 언어 {','.join(args.lang)} = {done[0]}번 견줌, 차이 {len(found)}, 흔들림 {len(shaky)}, {took:.0f}초")
    print("옛 판의 종료 코드: " + ", ".join(f"{k} {v}번" for k, v in sorted(codes.items(), key=lambda kv: -kv[1])))
    for k, lang, msg in harness[:20]:
        print(f"도구 오류: 사례 {k} [{lang}] {msg}")
    for k, lang, case, d in sorted(found, key=lambda t: (t[0], t[1])):
        print(f"\n### 시드 {seed} 사례 {k} [{lang}]")
        print("\n".join(describe(case)))
        print(f"    다시: bindiff_fuzz.py --old {old} --new {new} --commit {args.commit} --seed {seed} --case {k} --lang {lang}")
        print("\n".join("    | " + x for x in d[:30]))
    for k, lang, case, d in sorted(shaky, key=lambda t: (t[0], t[1])):
        print(f"\n### 흔들림 — 시드 {seed} 사례 {k} [{lang}]: 다시 돌리니 차이가 사라졌다. 원인을 찾을 때까지 실패로 센다")
        print("\n".join(describe(case)))
        print("\n".join("    | " + x for x in d[:12]))
    if harness or done[0] == 0:
        # 아무것도 안 견준 판(`--cases 0`, 빈 `--lang`)은 "차이 0" 이 아니다.
        return 2
    return 1 if found or shaky else 0


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--old", required=True, type=Path)
    ap.add_argument("--new", required=True, type=Path)
    ap.add_argument("--seed", type=int, default=None, help="없으면 시각으로 정하고 첫 줄에 찍는다")
    ap.add_argument("--cases", type=int, default=DEFAULT_CASES, help="언어마다 몇 사례")
    ap.add_argument("--case", type=int, default=None, help="이 번호의 사례 하나만")
    ap.add_argument("--lang", default="en,ko", type=lambda s: s.split(","))
    ap.add_argument("--commit", default="3f569736", help="입력 저장소로 쓸 이 저장소의 커밋")
    ap.add_argument("--jobs", type=int, default=DEFAULT_JOBS)
    ap.add_argument("--work", type=Path, help="자리를 둘 디렉터리. 없으면 임시 디렉터리를 쓰고 끝에 지운다")
    ap.add_argument("--keep", action="store_true", help="끝나도 자리를 안 지운다")
    args = ap.parse_args(argv)
    if args.seed is None:
        args.seed = int(time.time())
    old, new = args.old.resolve(), args.new.resolve()
    for b in (old, new):
        if not os.access(b, os.X_OK):
            say(f"bindiff_fuzz: {b} 를 실행할 수 없다")
            return 2
    print(f"시드 {args.seed}  (같은 시드와 --case 번호로 그 사례가 다시 돈다)", flush=True)
    say(f"old {old}")
    say(f"new {new}")
    tmp = None
    if args.work:
        work = args.work.resolve()
        work.mkdir(parents=True, exist_ok=True)
    else:
        tmp = tempfile.mkdtemp(prefix="bindiff-fuzz-")
        work = Path(tmp)
    try:
        above = bindiff.tracker_above(work)
        if above:
            say(f"bindiff_fuzz: 자리({work}) 위에 트래커({above / '.moai'})가 있다 — `-C {{home}}` 같은 사례의 쓰기가 거기에 든다. 트래커 밖의 --work 를 준다")
            return 2
        template = Template(work, args.commit, old)
        template.build()
        bindiff.HOOK_BIN[:] = [str(old)]
        gen = build_gen(old, template)
        if not gen.dep_cases or len(gen.paths) < 10:
            say("bindiff_fuzz: 도움말에서 명령 나무를 못 읽었다")
            return 2
        r = random.Random(f"{args.seed}:offsets")
        gen.help_off = r.randrange(len(gen.paths))
        gen.dep_off = r.randrange(len(gen.dep_cases))
        return fuzz(args, old, new, work, template, gen)
    except SystemExit as e:
        say(str(e))
        return 2
    finally:
        if tmp and not args.keep:
            shutil.rmtree(tmp, ignore_errors=True)


DEFAULT_CASES = 10000
DEFAULT_JOBS = min(8, os.cpu_count() or 1)

if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
