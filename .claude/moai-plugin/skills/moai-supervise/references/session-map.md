# Session map

```sh
python3 - <<'PY'
import glob, json, os, re, subprocess, sys
def out(*cmd, **env):
    try:
        return subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, universal_newlines=True, env=dict(os.environ, **env), timeout=10).stdout.strip()
    except (OSError, subprocess.TimeoutExpired):
        return ""
linux = os.path.isdir("/proc/self")
def stat(pid):
    try:
        with open(f"/proc/{pid}/stat") as f:
            text = f.read()
        return text[text.rindex(")") + 2:].split()
    except (OSError, ValueError):
        return []
def started(pid):
    if linux:
        fields = stat(pid)
        return fields[19] if len(fields) > 19 else ""
    return " ".join(out("ps", "-p", str(pid), "-o", "lstart=", LC_ALL="C", TZ="UTC").split())
def device(path):
    try:
        return os.stat(path).st_rdev if path else 0
    except OSError:
        return 0
def tty(pid):
    if linux:
        try:
            return int(stat(pid)[4]) & 0xFFFFFFFF
        except (IndexError, ValueError):
            return 0
    t = out("ps", "-p", str(pid), "-o", "tty=")
    return device("/dev/" + t) if t.strip("?") else 0
def nested_multiplexer(pid):
    # Inspect process names and parents only, never another process's environment.
    seen = set()
    while pid > 1 and pid not in seen:
        seen.add(pid)
        line = out("ps", "-p", str(pid), "-o", "ppid=", "-o", "comm=")
        fields = line.split(None, 1)
        if len(fields) != 2:
            return True
        try:
            pid = int(fields[0])
        except ValueError:
            return True
        name = os.path.basename(fields[1].strip()).lower()
        if name in ("cmux", "cmux dev"):
            break
        if name in ("screen", "tmux", "zellij") or name.startswith("screen-") or name.startswith("tmux:"):
            return True
    return False
mine = os.environ.get("TMUX")
if mine and mine.startswith("/tmp/cmux-"):
    mine = None
cmux = not mine and bool(os.environ.get("CMUX_SURFACE_ID"))
surfaces = {}
def walk(o):
    if isinstance(o, dict):
        for p in o.get("cmux_process_pids") or []:
            surfaces[p] = o.get("id")
        o = list(o.values())
    for v in o if isinstance(o, list) else []:
        walk(v)
if cmux:
    try:
        walk(json.loads(out("cmux", "--json", "--id-format", "both", "top", "--all")))
    except ValueError:
        print("cmux top gave no answer, so no session is matched to a surface", file=sys.stderr)
rows = []
for path in sorted(glob.glob(os.path.expanduser("~/.claude/sessions/*.json"))):
    try:
        with open(path) as f:
            r = json.load(f)
        pid = int(r["pid"])
    except (OSError, ValueError, KeyError, TypeError):
        continue
    start = started(pid)
    alive = bool(start) and start == " ".join(str(r.get("procStart")).split())
    told = r.get("tmux") or ""
    tmux = told if re.search(r":@\d{1,6}\.%\d{1,6}$", told) else ""
    pane = tmux.rpartition(".")[2] if tmux else ""
    if r.get("entrypoint") == "remote_mobile":
        pane = ""
    elif alive and (mine or cmux) and r.get("kind") not in (None, "interactive"):
        pane = ""
    elif alive and pane and mine:
        here = tty(pid)
        if not here or here != device(out("tmux", "display", "-p", "-t", pane, "#{pane_tty}")):
            pane = ""
    elif alive and cmux:
        pane = surfaces.get(pid)
        if (told and not told.startswith("cmux:")) or (pane and nested_multiplexer(pid)):
            pane = ""
    rows.append((alive, pane, r))
if rows and not any(alive for alive, _, _ in rows):
    sys.exit("no record reads alive, not even this session's own: this shell cannot read process start times (a sandbox, or no ps), so act on no row")
claimed = [pane for alive, pane, _ in rows if alive and pane and (mine or cmux)]
for alive, pane, r in rows:
    if alive and claimed.count(pane) > 1:
        pane = ""
    saycode = "saycode" if r.get("entrypoint") == "remote_mobile" else ""
    cols = [r.get("name"), pane, r.get("status"), r.get("cwd"), r.get("sessionId"), saycode]
    print("\t".join(["alive" if alive else "dead"] + [str(c or "-") for c in cols]))
PY
```
