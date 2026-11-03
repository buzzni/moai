#!/usr/bin/env python3
"""두 moai 바이너리에 같은 입력을 주고, 나온 것이 바이트째 같은지 견준다 (moai-72qj.j6p).

    scripts/bindiff.py --old <옛 바이너리> --new <새 바이너리>
    scripts/bindiff.py --old A --new A                  옛 대 옛 — 도구를 믿기 전에 먼저 돌린다
    scripts/bindiff.py --old A --new B --only '^show'   이름이 맞는 사례만
    scripts/bindiff.py latency --old A --new B -n 300   시작 지연

**왜 있나.** 동작을 바꾸지 않는다고 약속한 리팩터(lib/bin 가르기, 가시성 열기)의 머지 관문이다.
사람의 눈 대신 "같은 입력에 같은 출력" 을 기계로 본다. 견주는 것은 넷이다 — stdout, stderr, 종료
코드, 그리고 그 명령이 쓴 파일 전부(저장소의 바뀐 파일·`.git` 의 설정·`HOME`·`TMPDIR`·`MOAI_CTL_DIR`).

**흔들림은 가리지 않고 원인을 고정한다.** 시계는 `MOAI_NOW`, 자리는 같은 경로, 파일의 mtime 은
매 사례 앞에 한 값으로 맞춘다. 사람 목소리·언어·색·폭도 환경으로 고정한다. 가리는 것은 하나뿐이다 —
`add` 가 새로 지은 id. id 는 제목·나노초·pid 로 지어(`src/id.rs` 의 `seed`) 부를 때마다 다르다.

**옛 판과 새 판은 같은 자리에서 차례로 돈다.** 출력에 저장소 경로가 찍히니 자리가 다르면 그것부터
갈린다. 그래서 사례 하나마다 자리를 되돌리고 옛 판, 다시 되돌리고 새 판을 돌린다. 여러 사례를
동시에 돌릴 때는 일꾼마다 자리를 따로 둔다 — 한 사례의 두 판은 늘 한 자리를 쓴다.

차등 퍼징(`scripts/bindiff_fuzz.py`, moai-72qj.hjo)이 이 파일의 `Sandbox`·`Case`·`compare` 를 그대로
쓴다. 새 의존성은 없다(파이썬 표준 라이브러리).
"""

from __future__ import annotations

import argparse
import concurrent.futures as futures
import dataclasses
import difflib
import json
import os
import re
import shutil
import signal
import statistics
import subprocess
import sys
import tempfile
import threading
import time
import urllib.request
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

# 고정 시각. 입력 저장소의 마지막 커밋보다 뒤여야 "n일 전" 이 음수가 안 된다.
NOW = "2026-11-03T03:00:00Z"
# 파일 mtime 을 맞출 값(초). 위 시각과 같은 순간이다.
MTIME = 1793674800
ACTOR = "Diff Test (diff@example.com)"
OTHER = "Other Person (other@example.com)"
LANGS = ("en", "ko")
TIMEOUT = 20.0

ID_RE = re.compile(rb"\bmoai-[0-9a-z]{4}(?:\.[0-9a-z]{3})*\b")


def say(*parts: object) -> None:
    print(*parts, file=sys.stderr, flush=True)


def git(cwd: Path, *args: str, check: bool = True) -> str:
    env = clean_git_env()
    p = subprocess.run(["git", *args], cwd=cwd, env=env, capture_output=True, text=True)
    if check and p.returncode != 0:
        raise SystemExit(f"bindiff: git {' '.join(args)} 이 실패했다 ({cwd}):\n{p.stderr}")
    return p.stdout


def clean_git_env() -> dict[str, str]:
    """git 을 부를 때의 환경. 사용자의 전역 설정과 훅의 `GIT_*` 를 안 물려받는다."""
    env = {k: v for k, v in os.environ.items() if not k.startswith("GIT_")}
    env.update(
        GIT_CONFIG_NOSYSTEM="1",
        GIT_CONFIG_GLOBAL="/dev/null",
        GIT_AUTHOR_NAME="bindiff",
        GIT_AUTHOR_EMAIL="bindiff@example.com",
        GIT_COMMITTER_NAME="bindiff",
        GIT_COMMITTER_EMAIL="bindiff@example.com",
        GIT_AUTHOR_DATE="2026-11-03T02:00:00Z",
        GIT_COMMITTER_DATE="2026-11-03T02:00:00Z",
        LC_ALL="C",
    )
    return env


# ── 사례 ──────────────────────────────────────────────────────────────


@dataclasses.dataclass
class Case:
    """명령 하나. `args` 는 `moai` 뒤의 인자이고 `-C <저장소>` 는 붙이지 않는다 — 돌리는 자리가 저장소다."""

    name: str
    args: list[str]
    stdin: bytes | None = None
    env: dict[str, str | None] = dataclasses.field(default_factory=dict)
    # 저장소 안의 어디서 부르나(상대 경로). None 이면 저장소 뿌리.
    cwd: str | None = None
    # 부르기 전에 자리에 해 둘 것. 옛 판·새 판 앞에서 똑같이 돈다.
    before: object | None = None
    timeout: float = TIMEOUT


@dataclasses.dataclass
class Outcome:
    code: int | str
    stdout: bytes
    stderr: bytes
    files: dict[str, bytes | None]


# ── 자리 ──────────────────────────────────────────────────────────────


class Template:
    """모든 자리가 복제해 쓰는 입력 저장소. 한 번 지어 커밋하고, 그 커밋에서 자리마다 clone 한다.

    입력의 줄(id 포함)이 모든 자리에서 같아야 사례가 id 를 이름으로 댈 수 있다 — 그래서 꾸밈은
    여기서 한 번, `setup_bin` 으로 돈다. 그 바이너리가 무엇이든 두 판이 받는 입력은 같다.
    """

    def __init__(self, work: Path, commit: str, setup_bin: Path):
        self.work = work
        self.path = work / "template"
        self.commit = commit
        self.setup_bin = setup_bin
        self.ids: dict[str, str] = {}

    def build(self) -> None:
        if self.path.exists():
            shutil.rmtree(self.path)
        git(self.work, "clone", "-q", "--shared", "--no-checkout", str(REPO), str(self.path))
        git(self.path, "checkout", "-q", "-B", "bindiff", self.commit)
        git(self.path, "remote", "remove", "origin")
        home = self.work / "template-home"
        shutil.rmtree(home, ignore_errors=True)
        (home / ".config").mkdir(parents=True)
        self.ids = decorate(self.path, home, self.setup_bin)
        git(self.path, "add", "-A")
        git(self.path, "commit", "-q", "-m", "bindiff: 입력 꾸밈")
        (self.work / "ids.json").write_text(json.dumps(self.ids, indent=1, ensure_ascii=False))
        shutil.rmtree(home, ignore_errors=True)


def decorate(repo: Path, home: Path, moai: Path) -> dict[str, str]:
    """칸마다·종류마다 한 줄씩은 서도록 입력을 꾸민다. 받은 id 를 이름으로 돌려준다.

    입력 저장소(이 저장소의 정한 커밋)는 열린 일이 거의 없다 — 닫힌 것만으로는 `ready`·`prime`·
    경고·미룸·막힘 갈래가 안 돈다.
    """
    env = base_env(home, repo.parent / "template-tmp", repo.parent / "template-ctl", "en")
    for d in ("template-tmp", "template-ctl"):
        (repo.parent / d).mkdir(exist_ok=True)

    def m(*args: str, stdin: str | None = None) -> str:
        p = subprocess.run(
            [str(moai), "--json", *args],
            cwd=repo,
            env=env,
            input=stdin.encode() if stdin is not None else None,
            capture_output=True,
        )
        if p.returncode != 0:
            raise SystemExit(f"bindiff: 꾸밈 `moai {' '.join(args)}` 가 실패했다:\n{p.stderr.decode()}")
        return p.stdout.decode()

    def made(out: str) -> list[str]:
        v = json.loads(out)
        rows = v if isinstance(v, list) else [v]
        return [r["id"] for r in rows]

    ids: dict[str, str] = {}
    ms = made(m("milestone", "add", "v9.9.9", "--start", "2026-10-30", "--due", "2026-11-20"))[0]
    ids["milestone"] = ms
    plan = (
        "# 차등 시험 에픽\n"
        "- [p1] 첫 일 #parser @2026-11-05\n"
        "- [p2] 둘째 일 #ui\n"
        "- [p2] 셋째 일\n"
        "- [p3] 넷째 일 #parser\n"
        "- [p1] 다섯째 일\n"
    )
    out = made(m("add", "--from", "-", "--milestone", ms, "--due", "2026-11-15", "--body", "왜 한 묶음인가: 시험", stdin=plan))
    ids["epic"], ids["m1"], ids["m2"], ids["m3"], ids["m4"], ids["m5"] = out[:6]
    m("mv", ids["m1"], "in_progress", "--from", "todo")
    m("mv", ids["m2"], "in_progress")
    m("mv", ids["m2"], "review", "-m", "리뷰로 보낸다")
    m("link", ids["m3"], "--blocks", ids["m4"])
    m("defer", ids["m5"], "-m", "다음 분기")
    m("note", ids["m1"], "model: anthropic/opus-5.5 (high — 쓰기 경로)")
    m("note", ids["m1"], "Next: 다음 걸음")
    ids["bug"] = made(m("add", "파서가 BOM 에서 죽는다", "-t", "bug", "-p", "0", "--due", "2026-11-04", "-b", "## 무엇\n\n- BOM\n"))[0]
    ids["theirs"] = made(m("add", "남의 일", "-a", OTHER, "-p", "1"))[0]
    ids["unowned"] = made(m("add", "담당 없는 일", "-a", "none", "-p", "2"))[0]
    ids["child"] = made(m("add", "자식 일", "--parent", ids["bug"]))[0]
    ids["late"] = made(m("add", "마감 지난 일", "--due", "2026-10-01", "-p", "3"))[0]
    ids["backlog"] = made(m("backlog", "add", "나중에 볼 생각", "-b", "떠오른 자리: 시험"))[0]
    ids["backlog2"] = made(m("backlog", "add", "펼칠 생각"))[0]
    gone = made(m("add", "지울 일"))[0]
    m("rm", gone)
    ids["removed"] = gone
    ids["done"] = made(m("add", "끝난 일", "-p", "2"))[0]
    m("mv", ids["done"], "done", "-m", "끝")
    ids["review"] = made(
        m("add", "review — 첫 일", "-t", "review", "--parent", ids["m1"], "-b", "무엇을 보나: 시험")
    )[0]
    # 아카이브된 줄 하나. 입력 저장소의 아카이브에서 가장 앞의 줄을 쓴다.
    arch = repo / ".moai" / "archive"
    for f in sorted(arch.glob("*.jsonl")):
        line = f.read_text().splitlines()[0]
        ids["archived"] = json.loads(line)["id"]
        break
    # 그림 하나(아주 작은 PNG). `image` 와 본문의 그림 갈래가 돈다.
    png = repo.parent / "dot.png"
    png.write_bytes(tiny_png())
    m("image", "add", str(png), "--to", ids["m1"])
    return ids


def tiny_png() -> bytes:
    import struct
    import zlib

    def chunk(kind: bytes, data: bytes) -> bytes:
        c = struct.pack(">I", len(data)) + kind + data
        return c + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)

    raw = b"".join(b"\x00" + bytes([200, 30, 30] * 4) for _ in range(4))
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", 4, 4, 8, 2, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw))
        + chunk(b"IEND", b"")
    )


def base_env(home: Path, tmp: Path, ctl: Path, lang: str) -> dict[str, str]:
    """출력을 바꾸는 환경을 모두 못 박는다. 부르는 쪽의 환경은 하나도 물려받지 않는다."""
    return {
        "PATH": "/usr/bin:/bin",
        "HOME": str(home),
        "XDG_CONFIG_HOME": str(home / ".config"),
        "TMPDIR": str(tmp),
        "TZ": "UTC",
        "LC_ALL": "C",
        "LANG": "C",
        "TERM": "dumb",
        "COLUMNS": "100",
        "LINES": "40",
        "MOAI_NOW": NOW,
        "MOAI_LANG": lang,
        "MOAI_CTL_DIR": str(ctl),
        "MOAI_ACTOR": ACTOR,
        "MOAI_NO_UPDATE_CHECK": "1",
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_CONFIG_GLOBAL": str(home / ".gitconfig"),
        "GIT_AUTHOR_DATE": "2026-11-03T02:30:00Z",
        "GIT_COMMITTER_DATE": "2026-11-03T02:30:00Z",
    }


class Sandbox:
    """사례를 돌리는 자리 하나. 템플릿에서 clone 하고, 사례마다 처음 상태로 되돌린다."""

    def __init__(self, root: Path, template: Template):
        self.root = root
        self.repo = root / "repo"
        self.home = root / "home"
        self.tmp = root / "tmp"
        self.ctl = root / "ctl"
        self.template = template
        self.pristine: dict[str, bytes] = {}

    def build(self) -> None:
        shutil.rmtree(self.root, ignore_errors=True)
        self.root.mkdir(parents=True)
        git(self.root, "clone", "-q", "--shared", str(self.template.path), str(self.repo))
        git(self.repo, "remote", "remove", "origin")
        # `.git` 안에서 명령이 고칠 수 있는 파일. 되돌릴 때 이 값으로 덮는다.
        for rel in self.git_files():
            self.pristine[rel] = (self.repo / rel).read_bytes()
        self.reset()

    def git_files(self) -> list[str]:
        out = []
        for d in (".git/info", ".git/hooks"):
            for p in sorted((self.repo / d).rglob("*")):
                if p.is_file():
                    out.append(str(p.relative_to(self.repo)))
        return [".git/config", *out]

    def reset(self) -> None:
        git(self.repo, "reset", "-q", "--hard", "bindiff")
        git(self.repo, "clean", "-qfdx")
        for d in (".git/info", ".git/hooks"):
            for p in sorted((self.repo / d).rglob("*"), reverse=True):
                rel = str(p.relative_to(self.repo))
                if p.is_file() and rel not in self.pristine:
                    p.unlink()
        for rel, data in self.pristine.items():
            p = self.repo / rel
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_bytes(data)
        for d in (self.home, self.tmp, self.ctl):
            shutil.rmtree(d, ignore_errors=True)
            d.mkdir(parents=True)
        (self.home / ".config").mkdir()
        self.pin_mtimes()

    def pin_mtimes(self) -> None:
        """mtime 을 한 값으로. 되돌림이 다시 쓴 파일만 새 mtime 을 받아, 앞 사례가 무엇이었는지가 새지 않게."""
        for top in (self.repo, self.home):
            for dirpath, dirnames, filenames in os.walk(top):
                if dirpath == str(self.repo):
                    dirnames[:] = [d for d in dirnames if d != ".git"]
                for n in filenames:
                    try:
                        os.utime(os.path.join(dirpath, n), (MTIME, MTIME), follow_symlinks=False)
                    except OSError:
                        pass
                os.utime(dirpath, (MTIME, MTIME))

    def place(self, v: str) -> str:
        return v.replace("{repo}", str(self.repo)).replace("{home}", str(self.home)).replace("{tmp}", str(self.tmp))

    def env(self, lang: str, extra: dict[str, str | None]) -> dict[str, str]:
        env = base_env(self.home, self.tmp, self.ctl, lang)
        for k, v in extra.items():
            v = None if v is None else self.place(v)
            if v is None:
                env.pop(k, None)
            else:
                env[k] = v
        return env

    def snapshot(self) -> dict[str, bytes | None]:
        """명령이 남긴 것 전부. 값이 None 이면 디렉터리다(빈 디렉터리를 만든 것도 차이다)."""
        files: dict[str, bytes | None] = {}
        st = git(self.repo, "status", "--porcelain=v1", "-z", "--ignored", "-uall")
        for ent in st.split("\0"):
            if not ent:
                continue
            rel = ent[3:]
            p = self.repo / rel
            if p.is_symlink():
                files["repo/" + rel] = b"-> " + os.readlink(p).encode()
            elif p.is_file():
                files["repo/" + rel] = p.read_bytes()
            elif not p.exists():
                files["repo/" + rel] = b"<gone>"
        for rel in sorted({*self.pristine, *self.git_files()}):
            p = self.repo / rel
            data = p.read_bytes() if p.is_file() else b"<gone>"
            if data != self.pristine.get(rel):
                files["repo/" + rel] = data
        for name, top in (("home", self.home), ("tmp", self.tmp), ("ctl", self.ctl)):
            for p in sorted(top.rglob("*")):
                rel = f"{name}/{p.relative_to(top)}"
                if p.is_symlink():
                    files[rel] = b"-> " + os.readlink(p).encode()
                elif p.is_dir():
                    files[rel] = None
                elif p.is_socket() or p.is_fifo():
                    files[rel] = b"<special>"
                else:
                    files[rel] = p.read_bytes()
        return files

    def exe(self, moai: Path) -> Path:
        """두 판을 **같은 경로**에서 부른다. clap 의 `Usage:` 줄은 argv[0] 을, `init`·`skill install` 은
        `current_exe()` 를 찍어, 경로가 다르면 그것부터 갈린다. 하드 링크라 복사보다 싸다."""
        exe = self.root / "bin" / "moai"
        exe.parent.mkdir(exist_ok=True)
        exe.unlink(missing_ok=True)
        try:
            os.link(moai, exe)
        except OSError:
            shutil.copy2(moai, exe)
        return exe

    def run(self, moai: Path, case: Case, lang: str) -> Outcome:
        self.reset()
        moai = self.exe(moai)
        if callable(case.before):
            case.before(self)
            self.pin_mtimes()
        cwd = self.repo / case.cwd if case.cwd else self.repo
        # 자리표는 여기서 푼다 — 사례는 일꾼들이 나눠 쓰니 고쳐 쓰지 않는다.
        args = [self.place(a) for a in case.args]
        stdin = case.stdin
        if stdin is not None:
            stdin = self.place(stdin.decode()).encode()
        try:
            p = subprocess.run(
                [str(moai), *args],
                cwd=cwd,
                env=self.env(lang, case.env),
                input=stdin if stdin is not None else b"",
                capture_output=True,
                timeout=case.timeout,
                start_new_session=True,
            )
            code: int | str = p.returncode
            out, err = p.stdout, p.stderr
        except subprocess.TimeoutExpired as e:
            code, out, err = "timeout", e.stdout or b"", e.stderr or b""
        return Outcome(code, out, err, self.snapshot())


# ── 비교 ──────────────────────────────────────────────────────────────


class Masker:
    """`add` 가 새로 지은 id 를 `<new-N>` 으로. N 은 처음 나온 차례다 — stdout, stderr, 저널, 나머지 파일 순.

    저널은 쓴 차례대로 붙으니 거기서 처음 나온 차례가 만든 차례다. `issues.jsonl` 은 id 로 정렬돼
    있어 새 id 가 어디 끼는지가 판마다 다르다 — 그래서 그 파일은 아래 `compare_lines` 가 따로 본다.
    """

    def __init__(self, known: set[bytes]):
        self.known = known
        self.map: dict[bytes, bytes] = {}

    def learn(self, data: bytes) -> None:
        for m in ID_RE.finditer(data):
            tok = m.group(0)
            if tok not in self.known and tok not in self.map:
                self.map[tok] = b"<new-%d>" % (len(self.map) + 1)

    def mask(self, data: bytes) -> bytes:
        if not self.map:
            return data
        return ID_RE.sub(lambda m: self.map.get(m.group(0), m.group(0)), data)

    def fresh(self, line: bytes) -> bool:
        return any(m.group(0) in self.map for m in ID_RE.finditer(line))


def order_for_learning(files: dict[str, bytes | None]) -> list[str]:
    journals = sorted(k for k in files if "/journal" in k)
    rest = sorted(k for k in files if k not in journals)
    return journals + rest


# 입력 저장소에 줄로 선 id(스냅샷·아카이브). `known_ids` 가 채운다. 실행 뒤 스냅샷에 새로 선 줄의 id 는 저널·문서에
# 이미 적힌 글과 우연히 같아도 새 id 다 — 지운 줄의 id 나 문서의 예시 id 가 다시 지어질 수 있다(36^4 칸). 안 그러면
# 한쪽만 "아는 id" 라 가려지지 않고 차이로 나온다(퍼저 시드 101 사례 2972).
ROWS: set[bytes] = set()
ROW_ID = re.compile(rb'(?m)^\{"id":"(moai-[0-9a-z]{4}(?:\.[0-9a-z]{3})*)"')


def masked(o: Outcome, known: set[bytes]) -> tuple[Masker, Outcome]:
    rows: set[bytes] = set()
    for k, v in o.files.items():
        if v is not None and k.startswith("repo/.moai/") and k.endswith(".jsonl") and "journal" not in k:
            rows.update(ROW_ID.findall(v))
    mk = Masker(known - (rows - ROWS))
    mk.learn(o.stdout)
    mk.learn(o.stderr)
    for k in order_for_learning(o.files):
        v = o.files[k]
        if v is not None:
            mk.learn(v)
    return mk, o


def compare_text(label: str, a: bytes, b: bytes) -> list[str]:
    if a == b:
        return []
    diff = difflib.unified_diff(
        a.decode("utf-8", "replace").splitlines(),
        b.decode("utf-8", "replace").splitlines(),
        f"old/{label}",
        f"new/{label}",
        lineterm="",
        n=1,
    )
    lines = list(diff)
    if not lines:  # 끝 줄바꿈 하나만 다르다
        lines = [f"--- old/{label}", f"+++ new/{label}", f"  (끝 줄바꿈이 다르다: {a[-1:]!r} 대 {b[-1:]!r})"]
    if len(lines) > 60:
        lines = lines[:60] + [f"  … ({len(lines) - 60}줄 더)"]
    return lines


def compare_lines(label: str, a: bytes, b: bytes, ma: Masker, mb: Masker) -> list[str]:
    """새 id 가 든 줄은 가린 뒤 모음으로, 나머지 줄은 차례째 견준다.

    정렬된 파일에서 새 id 의 자리는 id 값이 정한다 — 그 자리만 차이로 안 친다. 새 id 가 없는 줄의
    차례는 그대로 본다.
    """
    al, bl = a.split(b"\n"), b.split(b"\n")
    a_old = b"\n".join(x for x in al if not ma.fresh(x))
    b_old = b"\n".join(x for x in bl if not mb.fresh(x))
    out = compare_text(label, a_old, b_old)
    a_new = sorted(ma.mask(x) for x in al if ma.fresh(x))
    b_new = sorted(mb.mask(x) for x in bl if mb.fresh(x))
    if a_new != b_new:
        out += compare_text(label + " (새 id 의 줄)", b"\n".join(a_new), b"\n".join(b_new))
    return out


# 훅이 쓰는 집기 기록(`<사용자 설정>/moai/picks/<저장소>/<세션>`)의 첫 칸은 벽시계 나노초다 — 같은
# 초에 집은 둘의 차례를 가르는 값이라 `MOAI_NOW` 를 따르지 않는다(`src/pulse.rs` 의 "wall-clock ordering
# nonce"). 시각 칸(셋째)은 `MOAI_NOW` 를 따르니 그대로 견준다. 가리는 것은 이 한 칸뿐이다.
PICK_NONCE = re.compile(rb"(?m)^[0-9]{10,20}\t")


def settle(o: Outcome) -> Outcome:
    files = {}
    for k, v in o.files.items():
        if v is not None and "/moai/picks/" in k:
            v = PICK_NONCE.sub(b"<nanos>\t", v)
        files[k] = v
    return Outcome(o.code, o.stdout, o.stderr, files)


# `--color always` 의 글은 id 앞에 색 끝의 `m` 이 붙어(`ESC[2mmoai-abcd`) 가리개의 `\b` 가 id 를 못 본다. 가리개를
# 넓히는 대신 색 끝 뒤에 단어 아닌 바이트 하나를 두 판에 똑같이 끼운다 — 다른 차이는 그대로 남는다.
SGR = re.compile(rb"(\x1b\[[0-9;]*m)")


def unglue(o: Outcome) -> Outcome:
    def sep(m: re.Match[bytes]) -> bytes:
        return m.group(1) + b"\x1f"

    return Outcome(o.code, SGR.sub(sep, o.stdout), SGR.sub(sep, o.stderr), o.files)


def compare(a: Outcome, b: Outcome, known: set[bytes]) -> list[str]:
    a, b = unglue(settle(a)), unglue(settle(b))
    ma, _ = masked(a, known)
    mb, _ = masked(b, known)
    out: list[str] = []
    if a.code != b.code:
        out.append(f"종료 코드: old {a.code} / new {b.code}")
    out += compare_text("stdout", ma.mask(a.stdout), mb.mask(b.stdout))
    out += compare_text("stderr", ma.mask(a.stderr), mb.mask(b.stderr))
    ka = {ma.mask(k.encode()).decode(): k for k in a.files}
    kb = {mb.mask(k.encode()).decode(): k for k in b.files}
    for k in sorted(set(ka) | set(kb)):
        if k not in ka:
            out.append(f"파일: new 만 {k}")
            continue
        if k not in kb:
            out.append(f"파일: old 만 {k}")
            continue
        va, vb = a.files[ka[k]], b.files[kb[k]]
        if va is None or vb is None:
            if (va is None) != (vb is None):
                out.append(f"파일: {k} 가 한쪽만 디렉터리다")
            continue
        out += compare_lines(k, va, vb, ma, mb)
    return out


# ── 사례 목록 ─────────────────────────────────────────────────────────


def cases(ids: dict[str, str], help_tree: list[list[str]]) -> list[Case]:
    """사람이 고른 묶음. 무작위로 넓히는 것은 `bindiff_fuzz.py` 다."""
    i = ids
    c: list[Case] = []

    def add(name: str, *args: str, **kw: object) -> None:
        c.append(Case(name, list(args), **kw))  # type: ignore[arg-type]

    def both(name: str, *args: str, **kw: object) -> None:
        add(name, *args, **kw)
        add(name + " --json", *args, "--json", **kw)

    # 도움말 — 모든 명령과 하위 명령
    add("--version", "--version")
    add("help", "--help")
    add("-h", "-h")
    add("help sub", "help", "show")
    for path in help_tree:
        add("help " + " ".join(path), *path, "--help")

    # 읽기
    add("bare", )
    for name, args in [
        ("status", ["status"]),
        ("status --worktree", ["status", "--worktree"]),
        ("ready", ["ready"]),
        ("ready --worktree", ["ready", "--worktree"]),
        ("prime", ["prime"]),
        ("stats", ["stats"]),
        ("config", ["config"]),
        ("models", ["models"]),
        ("writing", ["writing"]),
        ("keys", ["keys"]),
        ("project ls", ["project", "ls"]),
        ("backlog ls", ["backlog", "ls"]),
        ("init --check", ["init", "--check"]),
        ("show", ["show"]),
        ("show --all", ["show", "--all"]),
        ("show --tree", ["show", "--tree"]),
        ("show --tree --all", ["show", "--tree", "--all"]),
        ("show -s todo", ["show", "-s", "todo"]),
        ("show -s todo,review", ["show", "-s", "todo,review"]),
        ("show -t parser", ["show", "-t", "parser"]),
        ("show --no-tag parser", ["show", "--no-tag", "parser"]),
        ("show -t bug -p 0", ["show", "-t", "bug", "-p", "0"]),
        ("show -e epic", ["show", "-e", i["epic"]]),
        ("show -e none", ["show", "-e", "none"]),
        ("show --milestone", ["show", "--milestone", i["milestone"]]),
        ("show --parent", ["show", "--parent", i["bug"]]),
        ("show -a me", ["show", "-a", "me"]),
        ("show -a none", ["show", "-a", "none"]),
        ("show --type epic", ["show", "--type", "epic"]),
        ("show --type backlog", ["show", "--type", "backlog"]),
        ("show -g", ["show", "-g", "파서"]),
        ("show -g note", ["show", "-g", "다음 걸음"]),
        ("show --stale", ["show", "--stale", "1"]),
        ("show --since", ["show", "--since", "2026-11-01"]),
        ("show --created", ["show", "--created", "2026-11-01..2026-11-04"]),
        ("show --done", ["show", "--done", "2026-01-01..2026-11-04", "--archived"]),
        ("show --filter status", ["show", "--filter", "status=todo"]),
        ("show --filter due_on", ["show", "--filter", "due_on=2026-11-01~2026-11-30"]),
        ("show --sort", ["show", "--sort", "priority,updated:desc"]),
        ("show --sort due --reverse", ["show", "--sort", "due", "--reverse"]),
        ("show --sort status:asc,title", ["show", "--sort", "status:asc,title", "--all"]),
        ("show -n", ["show", "-n", "3", "--all"]),
        ("show --after", ["show", "--after", i["m1"], "--all"]),
        ("show --fields", ["show", "--fields", "id,title,status"]),
        ("show --archived", ["show", "--archived"]),
        ("show --archived -n", ["show", "--archived", "-n", "40"]),
        ("show --deferred", ["show", "--deferred"]),
        ("show --removed", ["show", "--removed"]),
        ("show --worktree", ["show", "--worktree"]),
        ("show issue", ["show", i["m1"]]),
        ("show issue --raw", ["show", i["bug"], "--raw"]),
        ("show epic", ["show", i["epic"]]),
        ("show epic --as-plan", ["show", i["epic"], "--as-plan"]),
        ("show milestone", ["show", i["milestone"]]),
        ("show backlog", ["show", i["backlog"]]),
        ("show archived", ["show", i["archived"]]),
        ("show review", ["show", i["review"]]),
        ("show theirs", ["show", i["theirs"]]),
        ("show deferred", ["show", i["m5"]]),
        ("show removed id", ["show", i["removed"]]),
        ("show kind", ["show", "epic"]),
        ("show old epic", ["show", "moai-8dap"]),
        ("show old milestone", ["show", "moai-r4jn"]),
        ("wiki ls", ["wiki", "ls"]),
        ("wiki show", ["wiki", "show", "cli"]),
        ("wiki search", ["wiki", "search", "worktree"]),
        ("wiki search ko", ["wiki", "search", "칸"]),
        ("due apply --dry-run", ["due", "apply", "--dry-run"]),
        ("archive --dry-run", ["archive", "--dry-run"]),
        ("backlog promote --dry-run", ["backlog", "promote", i["backlog2"], "--from", "-", "--dry-run"]),
        ("add --from --dry-run", ["add", "--from", "-", "--dry-run"]),
        ("epic show", ["epic", "show"]),
        ("milestone show", ["milestone", "show"]),
        ("issue show", ["issue", "show", "-s", "todo"]),
        ("ctl ls", ["ctl", "ls"]),
        ("ctl state", ["ctl", "state"]),
        ("web --dry-run", ["web", "--dry-run"]),
        ("web --dry-run --port 0", ["web", "--dry-run", "--port", "0"]),
    ]:
        stdin = None
        if "--from" in args:
            stdin = "# 펼친 에픽\n- [p1] 첫 일 #a\n- [p2] 둘째 일 @2026-11-10\n".encode()
        both(name, *args, stdin=stdin)

    # 탐색기의 기계 출력. 대화형 화면은 돌리지 않는다.
    both("tui", "tui")
    add("tui --json --deep", "tui", "--json", "--deep")
    add("tui --json --path", "tui", "--json", "--path", i["epic"])

    # 색. 파이프라 기본은 꺼진다 — 켠 판을 따로 본다.
    for name, args in [
        ("status", ["status"]),
        ("show --tree", ["show", "--tree", "--all"]),
        ("show issue", ["show", i["m1"]]),
        ("ready", ["ready"]),
        ("stats", ["stats"]),
    ]:
        add(name + " --color always", *args, "--color", "always")
    add("status --no-color", "status", "--no-color")

    # 하위 디렉터리에서 부름, -C
    add("status from subdir", "status", cwd="src")
    add("status -C", "-C", "{repo}", "status")
    add("status -C src", "-C", "{repo}/src", "status")
    add("status outside", "-C", "{home}", "status")
    add("ready outside", "-C", "{home}", "ready")

    # 사람
    add("status no actor", "status", env={"MOAI_ACTOR": None})
    add("add no actor", "add", "누가", env={"MOAI_ACTOR": None})
    add("note --user", "--user", "Kim (kim@example.com)", "note", i["m1"], "다른 사람")
    add("ready as other", "ready", env={"MOAI_ACTOR": OTHER})

    # 쓰기 — 사례마다 처음 상태에서
    w: list[tuple[str, list[str], bytes | None]] = [
        ("mv", ["mv", i["m3"], "in_progress"], None),
        ("mv --from", ["mv", i["m3"], "in_progress", "--from", "todo"], None),
        ("mv --from lost", ["mv", i["m3"], "in_progress", "--from", "review"], None),
        ("mv theirs", ["mv", i["theirs"], "in_progress", "--from", "todo"], None),
        ("mv --take", ["mv", i["theirs"], "in_progress", "--from", "todo", "--take", "-m", "사람이 허락"], None),
        ("mv unowned --take", ["mv", i["unowned"], "in_progress", "--take", "-m", "허락"], None),
        ("mv done", ["mv", i["m2"], "done", "-m", "끝냈다"], None),
        ("mv two", ["mv", i["m3"], i["m4"], "in_progress"], None),
        ("mv reopen", ["mv", i["done"], "todo", "--from", "done"], None),
        ("mv archived reopen", ["mv", i["archived"], "todo", "--from", "done"], None),
        ("mv blocked", ["mv", i["m4"], "in_progress"], None),
        ("mv epic", ["mv", i["epic"], "done"], None),
        ("edit title", ["edit", i["m3"], "--title", "바뀐 제목"], None),
        ("edit body", ["edit", i["m3"], "-b", "-"], "## 새 본문\n\n- 하나\n".encode()),
        ("edit tag", ["edit", i["m3"], "--tag", "parser", "--tag", "new"], None),
        ("edit untag", ["edit", i["m1"], "--untag", "parser"], None),
        ("edit epic none", ["edit", i["bug"], "-e", i["epic"]], None),
        ("edit member epic none", ["edit", i["m3"], "-e", "none"], None),
        ("edit milestone", ["edit", i["bug"], "--milestone", i["milestone"]], None),
        ("edit priority", ["edit", i["m3"], "-p", "0"], None),
        ("edit assignee", ["edit", i["m3"], "-a", OTHER], None),
        ("edit assignee none", ["edit", i["m3"], "-a", "none"], None),
        ("edit due", ["edit", i["m3"], "--due", "2026-11-10"], None),
        ("edit due beyond", ["edit", i["m3"], "--due", "2026-12-30"], None),
        ("edit due +3", ["edit", i["m3"], "--due", "+3"], None),
        ("edit due none", ["edit", i["m1"], "--due", "none"], None),
        ("edit start", ["edit", i["m3"], "--start", "2026-11-04"], None),
        ("edit shift", ["edit", i["m1"], "--shift", "2"], None),
        ("edit shift-due", ["edit", i["m1"], "--shift-due", "-1"], None),
        ("edit if-unplanned", ["edit", i["m1"], "--due", "2026-11-06", "--if-unplanned"], None),
        ("note", ["note", i["m3"], "찾은 것"], None),
        ("note -b -", ["note", i["m3"], "-b", "-"], "여러 줄\n둘째 줄 'quote'\n".encode()),
        ("note model", ["note", i["m3"], "model: anthropic/sonnet-5.5 (medium — 시험)"], None),
        ("defer", ["defer", i["m3"], "-m", "나중에"], None),
        ("defer --from", ["defer", i["m3"], "-m", "나중에", "--from", "todo"], None),
        ("defer --undo", ["defer", i["m5"], "--undo"], None),
        ("defer epic", ["defer", i["epic"], "-m", "통째로"], None),
        ("link", ["link", i["m1"], "--blocks", i["m3"]], None),
        ("link clear", ["link", i["m3"], "--blocks", i["m4"], "--clear"], None),
        ("rm", ["rm", i["m3"]], None),
        ("rm epic", ["rm", i["epic"]], None),
        ("read", ["read", i["m1"]], None),
        ("read all", ["read", "--all"], None),
        ("due apply", ["due", "apply"], None),
        ("archive", ["archive"], None),
        ("archive --drop", ["archive", "--drop", i["archived"]], None),
        ("backlog add", ["backlog", "add", "새 생각"], None),
        ("backlog add -b -", ["backlog", "add", "긴 생각", "-b", "-"], "본문 줄\n".encode()),
        ("backlog promote", ["backlog", "promote", i["backlog2"], "--from", "-"], "# 펼친 에픽\n- [p1] 하나\n- [p2] 둘\n".encode()),
        ("backlog promote -e", ["backlog", "promote", i["backlog2"], "-e", i["epic"], "--from", "-"], "- [p1] 하나\n".encode()),
        ("add", ["add", "새 일"], None),
        ("add full", ["add", "새 버그", "-t", "bug", "-p", "1", "-e", i["epic"], "--due", "2026-11-05", "-b", "본문"], None),
        ("add -q", ["add", "조용한 일", "-q"], None),
        ("add -b -", ["add", "본문 일", "-b", "-"], "본문\n".encode()),
        ("add --parent", ["add", "자식", "--parent", i["m1"]], None),
        ("add --type epic", ["add", "새 에픽", "--type", "epic"], None),
        ("add --from", ["add", "--from", "-"], "# 새 에픽 @2026-11-10\n- [p1] 하나 #a\n- [p2] 둘 @2026-11-08\n".encode()),
        ("add --from --var", ["add", "--from", "-", "--var", "v=1.2"], "# 릴리스 {{v}}\n- [p1] {{v}} 를 낸다\n".encode()),
        ("add -a other", ["add", "남에게", "-a", OTHER], None),
        ("add review", ["add", "review — 셋째", "-t", "review", "--parent", i["m3"], "-b", "관점"], None),
        ("add review no body", ["add", "review — 셋째", "-t", "review", "--parent", i["m3"]], None),
        ("epic add", ["epic", "add", "새 묶음"], None),
        ("milestone add", ["milestone", "add", "v10.0.0", "--due", "2026-12-01"], None),
        ("issue add", ["issue", "add", "이슈 동사"], None),
        ("image add", ["image", "add", "{repo}/../dot.png", "--to", i["m3"]], None),
        ("image add no file", ["image", "add", "{repo}/none.png", "--to", i["m3"]], None),
        ("project add", ["project", "add", "{repo}"], None),
        ("project ls after add", ["project", "ls"], None),
        ("init again", ["init"], None),
        ("merge-driver --install", ["merge-driver", "--install", "--as", "/usr/local/bin/moai-x"], None),
        ("skill install", ["skill", "install"], None),
        ("wiki learn", ["wiki", "learn", "--help"], None),
    ]
    for name, args, stdin in w:
        both(name, *args, stdin=stdin)

    # 오류 갈래
    for name, args in [
        ("show missing", ["show", "moai-zzzz"]),
        ("show bad id", ["show", "not-an-id!"]),
        ("mv missing", ["mv", "moai-zzzz", "done"]),
        ("mv bad column", ["mv", i["m3"], "nowhere"]),
        ("show bad status", ["show", "-s", "nowhere"]),
        ("show bad filter", ["show", "--filter", "nope=1"]),
        ("show bad sort", ["show", "--sort", "nope"]),
        ("show bad priority", ["show", "-p", "9"]),
        ("show bad date", ["show", "--created", "2026-13-01..x"]),
        ("edit bad due", ["edit", i["m3"], "--due", "2026-02-30"]),
        ("edit start after due", ["edit", i["m1"], "--start", "2026-11-30"]),
        ("add empty title", ["add", ""]),
        ("add no title", ["add"]),
        ("note empty", ["note", i["m3"], ""]),
        ("unknown command", ["frobnicate"]),
        ("unknown flag", ["status", "--frobnicate"]),
        ("bad color", ["status", "--color", "sometimes"]),
        ("-C missing", ["-C", "/nonexistent/x", "status"]),
        ("link self", ["link", i["m3"], "--blocks", i["m3"]]),
        ("defer missing -m", ["defer", i["m3"]]),
        ("ctl sort no window", ["ctl", "sort", "priority"]),
    ]:
        both(name, *args)

    def lock_held(sb: Sandbox) -> None:
        # 다른 프로세스가 락을 쥔 상태. flock 을 쥔 채 잠든 자식 하나를 남긴다 — 사례가 끝나면 죽인다.
        lock = sb.repo / ".moai" / "lock"
        lock.touch()
        p = subprocess.Popen(["flock", "-x", str(lock), "sleep", "30"], start_new_session=True)
        HELD.append(p)
        for _ in range(100):
            r = subprocess.run(["flock", "-xn", str(lock), "true"])
            if r.returncode != 0:
                break
            time.sleep(0.01)

    def broken_line(sb: Sandbox) -> None:
        f = sb.repo / ".moai" / "issues.jsonl"
        f.write_bytes(f.read_bytes() + b'{"id":"moai-brok","title":\n')

    def unknown_field(sb: Sandbox) -> None:
        f = sb.repo / ".moai" / "issues.jsonl"
        lines = f.read_bytes().split(b"\n")
        for k, line in enumerate(lines):
            if line.startswith(b'{"id":"' + i["m3"].encode() + b'"'):
                lines[k] = line[:-1] + b',"zz_future":{"a":[1,2]}}'
        f.write_bytes(b"\n".join(lines))

    for name, args, before in [
        ("mv lock held", ["mv", i["m3"], "in_progress"], lock_held),
        ("status broken line", ["status"], broken_line),
        ("show broken line", ["show", "--all"], broken_line),
        ("mv broken line", ["mv", i["m3"], "in_progress"], broken_line),
        ("mv unknown field", ["mv", i["m3"], "in_progress"], unknown_field),
        ("show unknown field", ["show", i["m3"]], unknown_field),
    ]:
        both(name, *args, before=before, timeout=12.0)

    c += hook_cases(ids)
    c += merge_cases()
    return c


HELD: list[subprocess.Popen] = []


def hook_cases(ids: dict[str, str]) -> list[Case]:
    """`moai hook` 에 사건을 stdin 으로. Claude 꼴은 손으로 짓고, codex·antigravity 는 기록한 자료를 쓴다."""
    out: list[Case] = []
    claude = {
        "session-start": {"session_id": "s1", "cwd": "{repo}", "hook_event_name": "SessionStart", "source": "startup"},
        "user-prompt-submit": {"session_id": "s1", "cwd": "{repo}", "prompt": "진행하자"},
        "stop": {"session_id": "s1", "cwd": "{repo}", "hook_event_name": "Stop"},
    }
    for ev, body in claude.items():
        out.append(Case(f"hook {ev}", ["hook", ev], stdin=json.dumps(body, ensure_ascii=False).encode()))
    tools = [
        ("read", {"tool_name": "Read", "tool_input": {"file_path": "{repo}/src/main.rs"}}),
        ("edit root", {"tool_name": "Edit", "tool_input": {"file_path": "{repo}/src/main.rs", "old_string": "a", "new_string": "b"}}),
        ("write outside", {"tool_name": "Write", "tool_input": {"file_path": "/tmp/x", "content": "x"}}),
        ("bash add", {"tool_name": "Bash", "tool_input": {"command": "moai add '딴 일'"}}),
        ("bash mv theirs", {"tool_name": "Bash", "tool_input": {"command": f"moai mv {ids['theirs']} in_progress"}}),
        ("bash mv theirs take", {"tool_name": "Bash", "tool_input": {"command": f"moai mv {ids['theirs']} in_progress --take -m ok"}}),
        ("bash review no body", {"tool_name": "Bash", "tool_input": {"command": f"moai add 'review — x' -t review --parent {ids['m1']}"}}),
        ("bash tmux kill", {"tool_name": "Bash", "tool_input": {"command": "tmux kill-server"}}),
        ("bash tmux -L kill", {"tool_name": "Bash", "tool_input": {"command": "tmux -L t kill-server"}}),
        ("bash sed -i", {"tool_name": "Bash", "tool_input": {"command": "sed -i s/a/b/ src/lib.rs"}}),
    ]
    for name, t in tools:
        body = {"session_id": "s1", "cwd": "{repo}", "hook_event_name": "PreToolUse", **t}
        out.append(Case(f"hook pre-tool-use {name}", ["hook", "pre-tool-use"], stdin=json.dumps(body, ensure_ascii=False).encode()))
    # 집은 것이 있을 때의 훅. 같은 세션이 먼저 집어 두면 규칙 1·2 가 돈다.
    def held(sb: Sandbox) -> None:
        env = sb.env("en", {"CLAUDE_CODE_SESSION_ID": "s1"})
        subprocess.run([HOOK_BIN[0], "mv", ids["m3"], "in_progress"], cwd=sb.repo, env=env, capture_output=True)

    for name, t in tools[:5]:
        body = {"session_id": "s1", "cwd": "{repo}", "hook_event_name": "PreToolUse", **t}
        out.append(
            Case(
                f"hook held pre-tool-use {name}",
                ["hook", "pre-tool-use"],
                stdin=json.dumps(body, ensure_ascii=False).encode(),
                before=held,
                env={"CLAUDE_CODE_SESSION_ID": "s1"},
            )
        )
    out.append(Case("hook held stop", ["hook", "stop"], stdin=b'{"session_id":"s1","cwd":"{repo}"}', before=held))
    out.append(Case("hook bad json", ["hook", "pre-tool-use"], stdin=b"{not json"))
    out.append(Case("hook empty", ["hook", "stop"], stdin=b""))
    out.append(Case("hook unknown event", ["hook", "nope"], stdin=b"{}"))
    for dialect in ("codex", "antigravity"):
        for f in sorted((REPO / "tests" / "hooks" / dialect).glob("*.json")):
            data = json.loads(f.read_text())
            data = swap_paths(data)
            ev = hook_event_of(f.stem)
            out.append(
                Case(
                    f"hook {dialect} {f.stem}",
                    ["hook", ev, "--dialect", dialect],
                    stdin=json.dumps(data, ensure_ascii=False).encode(),
                )
            )
    return out


# held() 가 쓰는 바이너리. 사례 앞 꾸밈은 두 판에 같아야 하니 옛 판으로 고정한다.
HOOK_BIN: list[str] = []


def swap_paths(v: object) -> object:
    if isinstance(v, dict):
        out = {}
        for k, x in v.items():
            if k in ("cwd", "Cwd") and isinstance(x, str):
                out[k] = "{repo}"
            elif k == "workspacePaths":
                out[k] = ["{repo}"]
            elif k in ("TargetFile", "AbsolutePath") and isinstance(x, str):
                out[k] = "{repo}/" + os.path.basename(x)
            else:
                out[k] = swap_paths(x)
        return out
    if isinstance(v, list):
        return [swap_paths(x) for x in v]
    return v


def hook_event_of(stem: str) -> str:
    for ev in ("session-start", "user-prompt-submit", "pre-tool-use", "stop", "pre-invocation"):
        if stem.startswith(ev):
            return "user-prompt-submit" if ev == "pre-invocation" else ev
    return "pre-tool-use"


def merge_cases() -> list[Case]:
    """`merge-driver <base> <ours> <theirs>`. 세 파일을 사례 앞에 tmp 에 깐다 — 답은 ours 에 쓰이니 그것도 견준다."""
    base = [
        '{"id":"moai-aaaa","title":"하나","status":"todo","created":"2026-11-01T00:00:00Z","updated":"2026-11-01T00:00:00Z"}',
        '{"id":"moai-bbbb","title":"둘","status":"todo","created":"2026-11-01T00:00:00Z","updated":"2026-11-01T00:00:00Z"}',
        '{"id":"moai-cccc","title":"셋","status":"todo","created":"2026-11-01T00:00:00Z","updated":"2026-11-01T00:00:00Z"}',
    ]

    def edit(lines: list[str], k: int, old: str, new: str) -> list[str]:
        out = list(lines)
        out[k] = out[k].replace(old, new)
        return out

    trios = {
        "clean": (base, edit(base, 0, "하나", "하나!"), edit(base, 2, "todo", "done")),
        "same row": (base, edit(base, 1, "둘", "둘-ours"), edit(base, 1, "둘", "둘-theirs")),
        "one side removes": (base, base[:1] + base[2:], edit(base, 1, "todo", "review")),
        "tags": (
            base,
            edit(base, 0, '"status"', '"tags":["a"],"status"'),
            edit(base, 0, '"status"', '"tags":["b"],"status"'),
        ),
        "garbage": (base, base + ["not json"], base),
    }
    out = []
    for name, (b, o, t) in trios.items():

        def lay(sb: Sandbox, b=b, o=o, t=t) -> None:
            for fn, ls in (("O", b), ("A", o), ("B", t)):
                (sb.tmp / fn).write_text("\n".join(ls) + "\n")

        out.append(
            Case(
                f"merge-driver {name}",
                ["merge-driver", "{tmp}/O", "{tmp}/A", "{tmp}/B", "7", ".moai/issues.jsonl"],
                before=lay,
            )
        )
    return out


def help_paths(moai: Path) -> list[list[str]]:
    """명령 나무를 도움말에서 읽는다 — `the_json_sweep_covers_every_command` 와 같은 길이다."""
    seen: list[list[str]] = []

    def walk(path: list[str]) -> None:
        p = subprocess.run([str(moai), *path, "--help"], capture_output=True, text=True, env={"PATH": "/usr/bin:/bin"})
        subs = []
        on = False
        for line in p.stdout.splitlines():
            if line.startswith("Commands:"):
                on = True
                continue
            if on:
                if not line.strip() or not line.startswith("  "):
                    on = False
                    continue
                name = line.split()[0]
                if name != "help":
                    subs.append(name)
        for s in subs:
            seen.append(path + [s])
            if len(path) < 3:
                walk(path + [s])

    walk([])
    return seen


# ── web ────────────────────────────────────────────────────────────────


def web_round(sb: Sandbox, moai: Path, lang: str, routes: list[str]) -> Outcome:
    """`web --port 0 --json` 을 띄워 첫 줄에서 주소를 읽고 GET 을 차례로 받는다. 끝나면 끈다."""
    sb.reset()
    moai = sb.exe(moai)
    p = subprocess.Popen(
        [str(moai), "web", "--port", "0", "--json"],
        cwd=sb.repo,
        env=sb.env(lang, {}),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        start_new_session=True,
    )
    body = []
    try:
        first = p.stdout.readline() if p.stdout else b""
        try:
            url = json.loads(first)["url"].rstrip("/")
        except (ValueError, KeyError):
            return Outcome("no url", first, b"", {})
        port = url.rsplit(":", 1)[1]
        body.append(first.replace(port.encode(), b"<port>"))
        for r in routes:
            req = urllib.request.Request(url + r, headers={"Host": "localhost"})
            try:
                with urllib.request.urlopen(req, timeout=TIMEOUT) as resp:
                    data = resp.read()
                    body.append(f"== GET {r} {resp.status} {resp.headers.get('Content-Type')}\n".encode() + data + b"\n")
            except urllib.error.HTTPError as e:
                body.append(f"== GET {r} {e.code}\n".encode() + e.read() + b"\n")
    finally:
        os.killpg(p.pid, signal.SIGTERM)
        try:
            _, err = p.communicate(timeout=5)
        except subprocess.TimeoutExpired:
            os.killpg(p.pid, signal.SIGKILL)
            _, err = p.communicate()
    return Outcome("served", b"".join(body), b"", sb.snapshot())


WEB_ROUTES = [
    "/",
    "/api/status",
    "/api/status?worktree=1",
    "/api/rows",
    "/api/backlog",
    "/api/tree",
    "/api/wiki",
    "/api/wiki-search?q=worktree",
    "/api/projects",
    "/api/nope",
]


# ── 돌림 ──────────────────────────────────────────────────────────────


def known_ids(template: Template) -> set[bytes]:
    known: set[bytes] = set()
    for p in (template.path / ".moai").rglob("*"):
        if p.is_file():
            known.update(ID_RE.findall(p.read_bytes()))
    # 이 저장소의 문서·소스에 적힌 id 도 새 것이 아니다(위키·도움말이 찍는다)
    for d in ("docs", "src", "AGENTS.md", "CLAUDE.md", "CHANGELOG.md"):
        p = template.path / d
        for f in [p] if p.is_file() else p.rglob("*") if p.exists() else []:
            if f.is_file():
                known.update(ID_RE.findall(f.read_bytes()))
    known.add(b"moai-zzzz")
    ROWS.clear()
    for f in [template.path / ".moai" / "issues.jsonl", *sorted((template.path / ".moai" / "archive").glob("*.jsonl"))]:
        if f.is_file():
            ROWS.update(ROW_ID.findall(f.read_bytes()))
    return known


def run_cases(args: argparse.Namespace, old: Path, new: Path, work: Path, template: Template, todo: list[Case]) -> int:
    known = known_ids(template)
    jobs = max(1, args.jobs)
    boxes = [Sandbox(work / f"box{k}", template) for k in range(jobs)]
    for b in boxes:
        b.build()
    pending = [(case, lang) for case in todo for lang in args.lang]
    lock = threading.Lock()
    failed: list[tuple[str, str, list[str]]] = []
    done = [0]
    # 옛 판의 종료 코드 분포. 사례가 다 오류로 끝나면 "차이 0" 이 아무것도 말하지 않는다.
    codes: dict[str, int] = {}
    started = time.time()

    def worker(box: Sandbox) -> None:
        while True:
            with lock:
                if not pending:
                    return
                case, lang = pending.pop(0)
            a = box.run(old, case, lang)
            reap()
            b = box.run(new, case, lang)
            reap()
            d = compare(a, b, known)
            with lock:
                done[0] += 1
                codes[str(a.code)] = codes.get(str(a.code), 0) + 1
                if d:
                    failed.append((case.name, lang, d))
                    say(f"차이: [{lang}] {case.name}")

    with futures.ThreadPoolExecutor(jobs) as ex:
        list(ex.map(worker, boxes))

    # web 은 자리 하나에서 차례로
    for lang in args.lang:
        if args.only and not re.search(args.only, "web"):
            break
        a = web_round(boxes[0], old, lang, WEB_ROUTES)
        b = web_round(boxes[0], new, lang, WEB_ROUTES)
        d = compare(a, b, known)
        done[0] += 1
        if d:
            failed.append(("web", lang, d))
            say(f"차이: [{lang}] web")

    took = time.time() - started
    report = work / "report.txt"
    with report.open("w") as f:
        for name, lang, d in failed:
            case = next((c for c in todo if c.name == name), None)
            f.write(f"### [{lang}] {name}\n")
            if case:
                f.write(f"    moai {' '.join(case.args)}\n")
                if case.env:
                    f.write(f"    env {case.env}\n")
                if case.stdin:
                    f.write(f"    stdin {case.stdin[:200]!r}\n")
            f.write("\n".join(d) + "\n\n")
    print(f"사례 {len(todo)}개 × 언어 {len(args.lang)} = {done[0]}번 견줌, 차이 {len(failed)}, {took:.0f}초")
    print("옛 판의 종료 코드: " + ", ".join(f"{k} {v}번" for k, v in sorted(codes.items())))
    if failed:
        print(f"차이의 전문: {report}")
        for name, lang, d in failed[: args.show]:
            print(f"\n### [{lang}] {name}")
            print("\n".join(d[:30]))
        return 1
    return 0


def reap() -> None:
    while HELD:
        p = HELD.pop()
        try:
            os.killpg(p.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        p.wait()


# ── 시작 지연 ─────────────────────────────────────────────────────────


def latency(args: argparse.Namespace, old: Path, new: Path, work: Path, template: Template) -> int:
    box = Sandbox(work / "lat", template)
    box.build()
    env = box.env("en", {})
    hook_in = json.dumps({"session_id": "s1", "cwd": str(box.repo), "tool_name": "Read", "tool_input": {"file_path": "/tmp/x"}}).encode()
    probes = [
        ("status --json", ["status", "--json"], b""),
        ("hook pre-tool-use", ["hook", "pre-tool-use"], hook_in),
        ("--version", ["--version"], b""),
    ]
    print(f"시작 지연 — 판마다 {args.n}번, 두 판을 번갈아. 값은 ms")
    print(f"{'probe':<20} {'old med':>8} {'old p10–p90':>14} {'new med':>8} {'new p10–p90':>14} {'차이':>7}")
    for name, a, stdin in probes:
        t = {"old": [], "new": []}
        for k in range(args.n):
            for side, b in (("old", old), ("new", new)) if k % 2 == 0 else (("new", new), ("old", old)):
                s = time.perf_counter()
                subprocess.run([str(b), *a], cwd=box.repo, env=env, input=stdin, capture_output=True)
                t[side].append((time.perf_counter() - s) * 1000)

        def q(xs: list[float]) -> tuple[float, float, float]:
            xs = sorted(xs)
            return statistics.median(xs), xs[len(xs) // 10], xs[len(xs) * 9 // 10]

        om, o1, o9 = q(t["old"])
        nm, n1, n9 = q(t["new"])
        print(f"{name:<20} {om:8.2f} {o1:6.2f}–{o9:<7.2f} {nm:8.2f} {n1:6.2f}–{n9:<7.2f} {nm - om:+7.2f}")
    return 0


# ── 입구 ──────────────────────────────────────────────────────────────


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("mode", nargs="?", default="cases", choices=["cases", "latency"])
    ap.add_argument("--old", required=True, type=Path)
    ap.add_argument("--new", required=True, type=Path)
    ap.add_argument("--commit", default="3f569736", help="입력 저장소로 쓸 이 저장소의 커밋")
    ap.add_argument("--lang", default="en,ko", type=lambda s: s.split(","))
    ap.add_argument("--only", help="이름이 이 정규식에 맞는 사례만")
    ap.add_argument("--jobs", type=int, default=min(8, os.cpu_count() or 1))
    ap.add_argument("--work", type=Path, help="자리를 둘 디렉터리. 없으면 임시 디렉터리를 쓰고 끝에 지운다")
    ap.add_argument("--keep", action="store_true", help="끝나도 자리를 안 지운다")
    ap.add_argument("--show", type=int, default=5, help="화면에 펼칠 차이의 수")
    ap.add_argument("-n", type=int, default=300, help="latency: 판마다 몇 번")
    args = ap.parse_args(argv)

    old, new = args.old.resolve(), args.new.resolve()
    for b in (old, new):
        if not os.access(b, os.X_OK):
            say(f"bindiff: {b} 를 실행할 수 없다")
            return 2
    say(f"old {subprocess.run([str(old), '--version'], capture_output=True, text=True).stdout.strip()}  {old}")
    say(f"new {subprocess.run([str(new), '--version'], capture_output=True, text=True).stdout.strip()}  {new}")

    tmp = None
    if args.work:
        work = args.work.resolve()
        work.mkdir(parents=True, exist_ok=True)
    else:
        tmp = tempfile.mkdtemp(prefix="bindiff-")
        work = Path(tmp)
    try:
        # 꾸밈은 옛 판이 한다 — 두 판이 받는 입력이 같으면 누가 꾸몄는지는 상관없다.
        template = Template(work, args.commit, old)
        template.build()
        HOOK_BIN[:] = [str(old)]
        if args.mode == "latency":
            return latency(args, old, new, work, template)
        todo = cases(template.ids, help_paths(old))
        if args.only:
            todo = [c for c in todo if re.search(args.only, c.name)]
        return run_cases(args, old, new, work, template, todo)
    finally:
        reap()
        if tmp and not args.keep:
            shutil.rmtree(tmp, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
