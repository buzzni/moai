#!/usr/bin/env python3
"""허용한 diff 만 지나게 하는 정적 검사 — 리팩터 에픽의 관문 G5 (moai-72qj.c62).

    scripts/check-refactor.py --base <커밋> --mode skeleton|surface [--head <rev>]
                              [--clippy-baseline <파일>]

`--base` 와 `--head`(기본 HEAD) 사이의 diff 가 허용 목록 안에 드는지 잰다. 목록 밖의 줄이
하나라도 있으면 그 파일과 줄을 찍고 1 로 끝난다. 통과하면 한 줄을 찍고 0 이다.

  skeleton  에픽 moai-72qj. `src/main.rs` 의 몸이 `src/lib.rs` 로 옮겨 가고, 바뀐 줄은
            `fn main() -> ExitCode {` -> `pub fn run() -> ExitCode {` 와, 제 소스를 경로로 읽는 시험의
            `"/src/main.rs"` -> `"/src/lib.rs"` 둘뿐이다.
  surface   에픽 moai-50p5. 가시성만 바뀐다 — `src/lib.rs` 의 `mod x;` -> `pub mod x;`
            와 모듈 안의 `pub(crate) ` -> `pub `. 맨 private -> `pub` 은 거절한다
            (glob import 가 새 이름을 끌어와 std prelude 의 같은 이름을 가리면, 컴파일되면서
            뜻이 바뀐다).

검사하는 것은 git 객체의 글이다. 그래서 작업 트리에 커밋 안 한 변경이 있으면(추적 파일의 변경,
무시 안 된 새 파일) 먼저 거절한다 — 재는 것과 머지되는 것이 같아야 한다.

`--clippy-baseline <파일>` 은 `${CARGO:-cargo} clippy --all-targets --workspace --locked` 를
돌려 경고 수를 파일 안의 수(`#` 줄·빈 줄을 뺀 첫 정수)와 견준다. 경고 수는 `--message-format=short`
출력에서 `<파일>:<줄>:<칸>: warning` 꼴의 서로 다른 줄을 센다 — 한 경고가 목표(lib·bin·test)마다
되풀이되어 찍혀도 한 번으로 센다. 다르면 거절한다. `--head` 가 HEAD 가 아니면 쓸 수 없다.

surface 의 줄 짝짓기 — 바뀐 `.rs` 마다 앞뒤를 줄 단위로 짝지어 다른 짝이 **정확히 하나의 허용된
치환**이어야 한다. `pub(crate) ` 를 `pub ` 로 바꾸면 줄이 7칸 짧아져 rustfmt 가 접어 둔 시그니처를
한 줄로 합친다(`cargo fmt --check` 가 시킨다) — 줄 수가 달라 짝이 안 맞는다. 그때만 앞뒤를 같은
폭(max_width=1000, 저장소의 나머지 rustfmt 설정 그대로)으로 다시 rustfmt 에 통과시켜 같은 엄한
짝짓기를 한 번 더 한다. rustfmt 는 문자열·주석의 내용을 안 바꾸므로 진짜 변경을 가리지 않는다.
그 길로 통과하는 파일은 머지될 글이 저장소 폭에서도 rustfmt 가 고정점인지(= `cargo fmt --check`
가 통과하는지) 함께 본다.
"""

import argparse
import difflib
import os
import re
import subprocess
import sys

# ---- 정해진 글 ---------------------------------------------------------------------------------

# skeleton 의 새 `src/main.rs`. 바이트째 같아야 한다. rustfmt(저장소 설정, edition 2024) 가 고정점이다.
GOLDEN_MAIN = (
    "//! 바이너리 진입점. 몸은 라이브러리(`src/lib.rs`)에 있다 — 여기는 넘겨주기만 한다.\n"
    "\n"
    "use std::process::ExitCode;\n"
    "\n"
    "fn main() -> ExitCode {\n"
    "    moai::run()\n"
    "}\n"
)

MAIN_OLD = "fn main() -> ExitCode {"
MAIN_NEW = "pub fn run() -> ExitCode {"
# 옮겨 간 시험 하나가 제 소스를 경로로 읽는다(`the_notices_never_reach_stdout_and_print_stands_last` —
# 경고 넷이 stdout 에 안 쓰는지를 글자로 훑는다). 몸이 lib.rs 로 가면 그 경로도 따라가야 한다 — 안 그러면
# 일곱 줄짜리 새 main.rs 를 읽고 `fn carried()` 가 없다며 진다(moai-72qj.nwl). 받는 둘째 줄은 이것 하나다.
SELF_OLD = '        let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/main.rs")).unwrap();'
SELF_NEW = '        let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs")).unwrap();'

# surface 에서 한 줄도 안 바뀌어야 하는 것. `#[path]` 로 시험 크레이트에도 드는 셋이 앞이다.
SURFACE_FROZEN = (
    "src/id.rs",
    "src/scratch.rs",
    "src/git_leaks.rs",
    "Cargo.toml",
    "Cargo.lock",
    "build.rs",
)
SKELETON_FROZEN = ("Cargo.lock", "build.rs")

VIS = ("pub(crate) ", "pub(super) ", "pub ")


class Refusal:
    """거절 사유를 모은다. 하나라도 있으면 끝에서 1 로 나간다."""

    def __init__(self):
        self.lines = []

    def add(self, text):
        self.lines.append(text)

    def at(self, path, lineno, text, why=""):
        tail = f"   <- {why}" if why else ""
        self.lines.append(f"  {path}:{lineno}: {text}{tail}")

    def __bool__(self):
        return bool(self.lines)


# ---- git ---------------------------------------------------------------------------------------


def git(root, *args, check=True):
    p = subprocess.run(["git", "-C", root, *args], capture_output=True)
    if check and p.returncode != 0:
        die(2, f"git {' '.join(args)} 가 실패했다: {p.stderr.decode(errors='replace').strip()}")
    return p


def die(code, msg):
    print(f"check-refactor: {msg}", file=sys.stderr)
    sys.exit(code)


def show(root, rev, path):
    """`rev:path` 의 글. 없으면 None."""
    p = git(root, "show", f"{rev}:{path}", check=False)
    if p.returncode != 0:
        return None
    return p.stdout.decode("utf-8", errors="surrogateescape")


def changed(root, base, head):
    """`{경로: 상태}` — A·M·D·T. 이름 바꿈은 D 와 A 로 푼다(짝을 지어 봐 주지 않는다)."""
    out = git(root, "diff", "--name-status", "-z", "--no-renames", "--no-ext-diff", base, head).stdout
    parts = out.decode("utf-8", errors="surrogateescape").split("\0")
    res = {}
    for i in range(0, len(parts) - 1, 2):
        res[parts[i + 1]] = parts[i][0]
    return res


def dirty(root):
    """커밋 안 한 변경 — 추적 파일의 변경과, 무시되지 않은 새 파일."""
    out = git(root, "status", "--porcelain=v1", "-z", "--untracked-files=all").stdout
    items = [x for x in out.decode("utf-8", errors="surrogateescape").split("\0") if x]
    return items


def lines_of(text):
    return text.split("\n")


# ---- skeleton ----------------------------------------------------------------------------------


def check_skeleton(root, base, head, files, bad):
    # 1. main.rs(기준) 의 몸이 lib.rs(머리) 로 옮겨 갔고, 바뀐 줄은 `fn main` 과 제 소스를 읽는 시험 한 줄뿐이다.
    old_main = show(root, base, "src/main.rs")
    new_lib = show(root, head, "src/lib.rs")
    if new_lib is None:
        bad.add(f"src/lib.rs 가 {head} 에 없다. skeleton 은 src/main.rs 의 몸이 src/lib.rs 로 옮겨 간 뒤에야 잴 수 있다.")
        return  # 나머지 검사는 이 사실 앞에서 소음이다
    elif old_main is None:
        bad.add(f"src/main.rs 가 기준 {base} 에 없다.")
    else:
        a, b = lines_of(old_main), lines_of(new_lib)
        if len(a) != len(b):
            bad.add(f"src/lib.rs 의 줄 수({len(b)})가 기준 src/main.rs({len(a)}) 와 다르다 — 줄을 더하거나 뺐다.")
        diffs = [i for i in range(min(len(a), len(b))) if a[i] != b[i]]
        # 받는 짝은 둘이고, 기준에 그 줄이 있으면 둘 다 꼭 바뀌어야 한다.
        for old, new in ((MAIN_OLD, MAIN_NEW), (SELF_OLD, SELF_NEW)):
            want = sum(1 for x in a if x == old)
            got = sum(1 for i in diffs if (a[i], b[i]) == (old, new))
            if want != 1:
                bad.add(f"기준 src/main.rs 에 `{old.strip()}` 가 {want}번 있다 — 한 번이어야 이 검사가 선다.")
            elif got != 1:
                bad.add(f"src/lib.rs: `{old.strip()}` 를 `{new.strip()}` 로 바꿔야 한다.")
        for i in diffs:
            if (a[i], b[i]) not in ((MAIN_OLD, MAIN_NEW), (SELF_OLD, SELF_NEW)):
                bad.at("src/lib.rs", i + 1, b[i], f"기준: {a[i]!r}")
        if len(a) != len(b):
            n = min(len(a), len(b))
            longer, name = (b, "src/lib.rs") if len(b) > len(a) else (a, "기준 src/main.rs")
            bad.at(name, n + 1, longer[n], "줄 수가 갈리는 첫 줄")

    # 2. 새 main.rs 는 정한 글과 바이트째 같다.
    head_main = show(root, head, "src/main.rs")
    if head_main is None:
        bad.add("src/main.rs 가 머리에 없다 — 정한 글을 써야 한다.")
    elif head_main != GOLDEN_MAIN:
        g, h = lines_of(GOLDEN_MAIN), lines_of(head_main)
        shown = 0
        for i in range(max(len(g), len(h))):
            gl = g[i] if i < len(g) else "(없음)"
            hl = h[i] if i < len(h) else "(없음)"
            if gl != hl:
                shown += 1
                if shown <= 10:
                    bad.at("src/main.rs", i + 1, hl, f"정한 글: {gl!r}")
        if shown > 10:
            bad.add(f"  src/main.rs: 정한 글과 다른 줄이 {shown}개 (앞 10개만 보인다)")

    # 3. Cargo.toml — 지운 줄이 없고, 더한 줄은 [lib]·doctest = false·빈 줄·# 주석뿐, 새 [lib] 절 하나.
    if "Cargo.toml" in files:
        check_cargo_toml(root, base, head, bad)
    else:
        # 빠지면 lib 이 된 순간 cargo test 가 문서 주석의 울타리를 doctest 로 돌린다.
        bad.add("Cargo.toml 이 안 바뀌었다 — [lib] 절의 doctest = false 가 있어야 한다.")

    # 4. 나머지 — 얼린 것은 그대로, 그 밖은 scripts/ 의 새 파일뿐.
    allowed_special = {"src/main.rs", "src/lib.rs", "Cargo.toml"}
    for path, st in sorted(files.items()):
        if path in allowed_special:
            continue
        if path in SKELETON_FROZEN or path.startswith("build/"):
            bad.at(path, 1, f"({st}) 바뀌면 안 되는 파일이다")
        elif path.startswith("scripts/") and st == "A":
            continue
        elif path.startswith("scripts/"):
            bad.at(path, 1, f"({st}) scripts/ 는 새 파일만 받는다 — 기존 파일은 안 고친다")
        else:
            bad.at(path, 1, f"({st}) 허용 목록 밖의 파일이다 (skeleton 은 scripts/ 의 새 파일만 더 받는다)")
    if files.get("src/lib.rs") not in (None, "A"):
        bad.at("src/lib.rs", 1, f"({files['src/lib.rs']}) 기준에 이미 있다 — 새 파일이어야 한다")
    if files.get("src/main.rs") not in (None, "M"):
        bad.at("src/main.rs", 1, f"({files['src/main.rs']}) 지우거나 새로 만들지 않고 고쳐야 한다")


HUNK = re.compile(r"^@@ -\d+(?:,\d+)? \+(\d+)(?:,\d+)? @@")


def check_cargo_toml(root, base, head, bad):
    out = git(root, "diff", "-U0", "--no-ext-diff", "--no-renames", base, head, "--", "Cargo.toml").stdout
    text = out.decode("utf-8", errors="surrogateescape")
    added = {}  # 머리 줄 번호 -> 글
    new_no = 0
    for ln in text.split("\n"):
        m = HUNK.match(ln)
        if m:
            new_no = int(m.group(1))
            continue
        if ln.startswith("---") or ln.startswith("+++") or ln.startswith("\\"):
            continue
        if ln.startswith("-"):
            bad.at("Cargo.toml", new_no, ln[1:], "지운 줄 — Cargo.toml 은 줄을 지우지 않는다")
        elif ln.startswith("+"):
            added[new_no] = ln[1:]
            new_no += 1
    for no, ln in sorted(added.items()):
        s = ln.strip()
        if s in ("[lib]", "doctest = false", "") or s.startswith("#"):
            continue
        bad.at("Cargo.toml", no, ln, "더한 줄은 [lib]·doctest = false·빈 줄·# 주석만 받는다")

    # 새 [lib] 절 하나. 줄 번호가 아니라 내용으로 센다.
    base_t = show(root, base, "Cargo.toml") or ""
    head_t = show(root, head, "Cargo.toml") or ""
    if re.search(r"^\[lib\]\s*$", base_t, re.M):
        bad.add("Cargo.toml: 기준에 이미 [lib] 절이 있다.")
        return
    hl = lines_of(head_t)
    libs = [i for i, ln in enumerate(hl) if ln.strip() == "[lib]"]
    if len(libs) != 1 or libs[0] + 1 not in added:
        bad.add("Cargo.toml: 더한 줄에 [lib] 절 머리가 정확히 하나여야 한다.")
        return
    # 그 절의 몸 — 다음 `[` 머리 앞까지. doctest = false 하나와 주석·빈 줄뿐.
    j = libs[0] + 1
    body = []
    while j < len(hl) and not hl[j].lstrip().startswith("["):
        body.append((j + 1, hl[j]))
        j += 1
    real = [(n, ln) for n, ln in body if ln.strip() and not ln.strip().startswith("#")]
    if [ln.strip() for _, ln in real] != ["doctest = false"]:
        for n, ln in real:
            if ln.strip() != "doctest = false":
                bad.at("Cargo.toml", n, ln, "[lib] 절에는 doctest = false 뿐이어야 한다")
        if not any(ln.strip() == "doctest = false" for _, ln in real):
            bad.add("Cargo.toml: [lib] 절에 doctest = false 가 없다.")
    elif real[0][0] not in added:
        bad.at("Cargo.toml", real[0][0], real[0][1], "doctest = false 가 더한 줄이 아니다")
    if sum(1 for ln in added.values() if ln.strip() == "doctest = false") != 1:
        bad.add("Cargo.toml: 더한 doctest = false 가 정확히 하나여야 한다.")


# ---- surface -----------------------------------------------------------------------------------

MOD_DECL = re.compile(r"^(\s*)mod (\w+);$")


def _in_code(a, idx):
    """`a[idx]` 가 주석·문자열 밖인가 — 어림이다. 의심스러우면 거짓이라 거절 쪽으로 틀린다."""
    pre = a[:idx]
    s = a.lstrip()
    if s.startswith("//") or s.startswith("*") or s.startswith("/*"):
        return False
    if "//" in pre or "/*" in pre:
        return False
    if pre.count('"') % 2 == 1:
        return False
    return True


def allowed_pair(a, b, is_lib):
    """(허용했는가, 거절 까닭). 줄 하나의 앞뒤가 정확히 허용된 치환인가."""
    if a == b:
        return True, ""
    if is_lib:
        m = MOD_DECL.match(a)
        if m and b == f"{m.group(1)}pub mod {m.group(2)};":
            return True, ""
    if "pub(crate) " in a and b == a.replace("pub(crate) ", "pub "):
        for m in re.finditer(re.escape("pub(crate) "), a):
            if not _in_code(a, m.start()):
                return False, "주석·문자열 안의 글이 바뀌었다"
        return True, ""
    sa, sb = strip_vis(a), strip_vis(b)
    if sa == sb:
        if "pub(super)" in a or "pub(super)" in b:
            return False, "pub(super) 는 바꾸지 않는다"
        if is_lib and MOD_DECL.match(a):
            return False, "mod 선언은 `mod x;` -> `pub mod x;` 꼴만 받는다"
        return False, "맨 private -> pub 이거나 허용 밖의 가시성 변경이다 (pub(crate) -> pub 만 받는다)"
    return False, "가시성 말고 다른 글이 바뀌었다"


def strip_vis(s):
    for v in VIS:
        s = s.replace(v, "")
    return s


def pair_all(a_lines, b_lines, is_lib):
    """줄 수가 같을 때 어긋난 짝 `[(줄 번호, 머리 글, 까닭)]`. 줄 수가 다르면 None."""
    if len(a_lines) != len(b_lines):
        return None
    bad = []
    for i, (a, b) in enumerate(zip(a_lines, b_lines)):
        ok, why = allowed_pair(a, b, is_lib)
        if not ok:
            bad.append((i + 1, b, why, a))
    return bad


def rustfmt(root, text, wide):
    """stdin -> rustfmt -> stdout. 실패하면 None. `wide` 면 max_width=1000."""
    cmd = ["rustfmt", "--edition", "2024", "--config-path", os.path.join(root, "rustfmt.toml")]
    if wide:
        cmd += ["--config", "max_width=1000"]
    p = subprocess.run(cmd, input=text.encode("utf-8", errors="surrogateescape"), capture_output=True, cwd=root)
    if p.returncode != 0:
        return None
    return p.stdout.decode("utf-8", errors="surrogateescape")


def check_surface_file(root, base, head, path, bad, paths_taken):
    old, new = show(root, base, path), show(root, head, path)
    if old is None or new is None:
        bad.at(path, 1, "기준이나 머리에서 글을 읽지 못했다")
        return
    is_lib = path == "src/lib.rs"
    a, b = lines_of(old), lines_of(new)
    raw = pair_all(a, b, is_lib)
    if raw == []:
        paths_taken.append((path, "줄 짝짓기"))
        return
    if raw is not None:
        # 줄 수가 같은데 어긋난 짝이 있다 — 접힘이 아니라 진짜 변경이다. 원본 줄 번호 그대로 말한다.
        for no, text, why, before in raw:
            bad.at(path, no, text, f"{why} / 기준: {before.strip()!r}")
        return
    # 줄 수가 다르다 -> pub(crate) 를 줄여 rustfmt 가 접은 것일 수 있다. 같은 폭(1000)으로 펴서 엄하게 한 번 더.
    wa, wb = rustfmt(root, old, True), rustfmt(root, new, True)
    norm = pair_all(lines_of(wa), lines_of(wb), is_lib) if wa is not None and wb is not None else None
    if norm is None:
        bad.at(path, 1, f"줄 수가 {len(a)} 에서 {len(b)} 로 바뀌었다", "rustfmt 정규화로도 짝이 안 맞는다")
        # 어디가 갈렸는지 — 가시성 표시를 지운 앞뒤를 difflib 로 견줘 머리 쪽 줄을 짚는다(앞 10개).
        sa, sb = [strip_vis(x) for x in a], [strip_vis(x) for x in b]
        shown = 0
        for tag, i1, i2, j1, j2 in difflib.SequenceMatcher(None, sa, sb, autojunk=False).get_opcodes():
            if tag == "equal":
                continue
            for j in range(j1, max(j2, j1 + 1)):
                if shown < 10:
                    shown += 1
                    bad.at(path, j + 1, b[j] if j < len(b) else "(줄이 빠졌다)", f"{tag}; 기준 {i1 + 1}~{i2}줄")
        return
    if norm:
        # 줄 수가 갈려 원본 줄 번호를 줄 수 없다 — 정규화한 글의 줄 번호다.
        for no, text, why, before in norm:
            bad.at(f"{path}[rustfmt 폭 1000 정규화한 글]", no, text, f"{why} / 기준: {before.strip()!r}")
        return
    # 정규화로 통과했다. 머지될 글이 저장소 폭에서 rustfmt 의 고정점이어야 "rustfmt 가 접었다" 가 성립한다
    # (정규화가 지우는 모양 차이 — 겹 공백 같은 것 — 를 숨겨 들이지 못하게 한다).
    fixed = rustfmt(root, new, False)
    if fixed == new:
        paths_taken.append((path, "rustfmt 정규화"))
        return
    fl = lines_of(fixed) if fixed is not None else []
    nl = lines_of(new)
    k = next((n for n in range(min(len(fl), len(nl))) if fl[n] != nl[n]), min(len(fl), len(nl)))
    bad.at(path, k + 1, nl[k] if k < len(nl) else "(끝)", "줄 수가 달라 정규화로 통과했으나 머리 글이 저장소 rustfmt 의 고정점이 아니다 — cargo fmt 를 돌린다")


def check_surface(root, base, head, files, bad):
    taken = []
    for path, st in sorted(files.items()):
        if path in SURFACE_FROZEN or path.startswith("build/"):
            bad.at(path, 1, f"({st}) 바뀌면 안 되는 파일이다")
        elif path.startswith("scripts/"):
            if st != "A":
                bad.at(path, 1, f"({st}) scripts/ 는 새 파일만 받는다")
        elif path == "tests/lib_api.rs":
            if st != "A":
                bad.at(path, 1, f"({st}) tests/lib_api.rs 는 새 파일로만 받는다")
        elif path == "CONTRIBUTING.md":
            if st != "M":
                bad.at(path, 1, f"({st}) CONTRIBUTING.md 는 고친 글만 받는다")
        elif path.startswith("src/") and path.endswith(".rs"):
            if st != "M":
                bad.at(path, 1, f"({st}) src/ 에 파일을 더하거나 지우지 않는다")
            else:
                check_surface_file(root, base, head, path, bad, taken)
        else:
            bad.at(path, 1, f"({st}) 허용 목록 밖의 파일이다")
    return taken


# ---- clippy ------------------------------------------------------------------------------------

WARN = re.compile(r"^\S.*?:\d+:\d+: warning")


def check_clippy(root, baseline_file, bad):
    try:
        with open(baseline_file, encoding="utf-8") as f:
            want = None
            for ln in f:
                s = ln.strip()
                if s and not s.startswith("#"):
                    want = int(s.split()[0])
                    break
    except (OSError, ValueError) as e:
        die(2, f"기준선 파일 {baseline_file} 을 읽지 못했다: {e}")
    if want is None:
        die(2, f"기준선 파일 {baseline_file} 에 수가 없다")
    cargo = os.environ.get("CARGO") or "cargo"
    p = subprocess.run(
        [cargo, "clippy", "--all-targets", "--workspace", "--locked", "--message-format=short"],
        capture_output=True,
        cwd=root,
    )
    out = (p.stdout + b"\n" + p.stderr).decode("utf-8", errors="replace")
    if p.returncode != 0:
        bad.add(f"clippy 가 코드 {p.returncode} 로 끝났다 (빌드 오류) — 경고 수를 재지 못했다.")
        return None
    seen = []
    for ln in out.split("\n"):
        if WARN.match(ln) and ln not in seen:
            seen.append(ln)
    got = len(seen)
    if got != want:
        bad.add(f"clippy 경고가 {got}개다 — 기준선 {baseline_file} 은 {want}개. 같아야 한다.")
        for ln in seen[:50]:
            bad.add(f"  {ln}")
    return got


# ---- main --------------------------------------------------------------------------------------


def main():
    ap = argparse.ArgumentParser(description="허용한 diff 만 지나게 한다 (moai-72qj.c62)")
    ap.add_argument("--base", required=True, help="기준 커밋")
    ap.add_argument("--mode", required=True, choices=["skeleton", "surface"])
    ap.add_argument("--head", default="HEAD", help="재는 쪽 (기본 HEAD)")
    ap.add_argument("--clippy-baseline", metavar="FILE", help="clippy 경고 수를 적은 파일")
    args = ap.parse_args()

    top = subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True)
    if top.returncode != 0:
        die(2, "git 저장소 안에서 불러야 한다")
    root = top.stdout.decode().strip()

    def rev(r):
        p = git(root, "rev-parse", "--verify", "--quiet", f"{r}^{{commit}}", check=False)
        if p.returncode != 0:
            die(2, f"{r} 를 커밋으로 읽지 못했다")
        return p.stdout.decode().strip()

    base, head = rev(args.base), rev(args.head)

    bad = Refusal()
    dirt = dirty(root)
    if dirt:
        print("check-refactor: 거절 — 커밋 안 한 변경이 있다. 재는 것은 머지될 커밋이어야 한다.", file=sys.stderr)
        for d in dirt:
            print(f"  {d}", file=sys.stderr)
        sys.exit(1)

    if args.clippy_baseline and head != rev("HEAD"):
        die(2, "--clippy-baseline 은 작업 트리를 재니 --head 가 HEAD 여야 한다")

    files = changed(root, base, head)
    taken = []
    if args.mode == "skeleton":
        check_skeleton(root, base, head, files, bad)
    else:
        taken = check_surface(root, base, head, files, bad)

    warned = None
    if args.clippy_baseline:
        warned = check_clippy(root, args.clippy_baseline, bad)

    if bad:
        print(f"check-refactor: 거절 ({args.mode}, 기준 {base[:10]}) — 허용 목록 밖의 변경:", file=sys.stderr)
        for ln in bad.lines:
            print(ln, file=sys.stderr)
        sys.exit(1)

    note = ""
    if taken:
        n_fmt = sum(1 for _, how in taken if how == "rustfmt 정규화")
        if n_fmt:
            note = f" (rustfmt 정규화 길: {', '.join(p for p, h in taken if h == 'rustfmt 정규화')})"
    if warned is not None:
        note += f" (clippy 경고 {warned}개, 기준선과 같다)"
    print(f"check-refactor: {args.mode} 통과 — 기준 {base[:10]}, 머리 {head[:10]}, 바뀐 파일 {len(files)}개{note}")


if __name__ == "__main__":
    main()
