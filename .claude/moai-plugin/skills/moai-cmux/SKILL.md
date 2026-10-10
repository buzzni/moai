---
name: moai-cmux
description: Use in Claude Code together with moai-supervise when the supervisor runs inside cmux (CMUX_SURFACE_ID is set and $TMUX is not) — to find a worker's surface from its ListAgents name, label its tab, clear a reported worker's window, deliver a message SendMessage could not, read why a worker stalled, and open new worker splits once the person says yes. Triggers on "worker surface", "worker tab", "open a worker", "clear the worker", "일꾼 탭", "일꾼 창 열어", "탭 비워".
---

# moai-cmux — the supervisor's hands on the workers' surfaces

This is the supervisor's (`moai-supervise`) companion **when it runs inside cmux** —
`CMUX_SURFACE_ID` is set in its shell and `$TMUX` is not. Inside tmux — tmux running in a cmux
tab included — what you type goes to tmux: that is `moai-tmux`, and none of this applies.
Without `CMUX_SURFACE_ID` none of this applies either: the supervisor works through messages and
the person alone. The cmux app here is **the person's own**: every workspace they have lives in
it, and every surface you touch is a tab they are looking at.

cmux's words: a window holds workspaces (the rows of its sidebar), a workspace holds panes (its
splits), and a pane holds surfaces (the tabs of that split). A surface is one terminal — what you
read and what you type into.

## First, the version

This needs cmux 0.65.0 or later. Look once, at the start of the round:

    cmux capabilities

If its `methods` do not list `surface.input_state`, the cmux here is older: tell the person once
that `moai-cmux` needs cmux 0.65.0 or later, and go on as if this skill were not here. Updating
restarts cmux, and with it every session inside — yours too — so when is theirs to choose.

## What never happens

- **No headless run.** A worker you open is an ordinary interactive `claude` the person can
  see and type into. Never `claude -p`, never a `--dangerously-*` flag, never
  `--permission-mode`. The moai binary launches and drives nothing; this skill is you
  typing cmux commands where the person can watch
- **Nothing is closed.** Never `close-surface`, `close-workspace` or `close-window`, never
  `respawn-pane`, never `pkill` or `killall` aimed at cmux. A worker that is done stays open;
  closing a tab is the person's
- **Never `--force`.** cmux refuses to type into a Claude Code box that holds someone's words or a
  dialog, and `--force` skips that refusal. A refusal is an answer — tell the person
- **Only a worker's surface.** Type only into a surface found through the session map below for
  a worker of this repository — never your own (`$CMUX_SURFACE_ID`), never a session of another
  repository. Every call names its surface by UUID, `--surface <surface>`: left out, cmux types
  into your own tab, and a `surface:N` name is handed out as asked for and does not outlive a
  restart
- **What the person looks at stays theirs.** No `focus-*`, no `select-workspace`, no
  `--focus true` — a split you open does not take focus
- **Never over the person's words.** If a worker's input box holds anything, do not type into
  it — tell the person
- **Never in their place.** A permission prompt or a question in a worker's tab is the
  person's to answer — you read it and tell them; you press no key
- **No polling.** Every look below is one look. A second one comes after your next step, as
  its own call — never a loop, never a `sleep`, never `wait-for`
- **The person's cmux settings stay theirs.** No `reload-config`, `themes`, `set-hook` or
  `bind-key`

## Which surface is which worker

`ListAgents` names a session; cmux needs a surface. Claude Code keeps one record per process
under `~/.claude/sessions/`, and the lines below print one row per record, tab-separated:

    state  name  pane  status  cwd  sessionId

```sh
python3 - <<'PY'
import glob, json, os, subprocess
def out(*cmd, **env):
    try:
        return subprocess.run(cmd, capture_output=True, text=True, env=dict(os.environ, **env)).stdout.strip()
    except OSError:
        return ""
def started(pid):
    if not os.path.isdir("/proc/self"):
        return " ".join(out("ps", "-p", str(pid), "-o", "lstart=", LC_ALL="C", TZ="UTC").split())
    try:
        with open(f"/proc/{pid}/stat") as f:
            stat = f.read()
        return stat[stat.rindex(")") + 2:].split()[19]
    except (OSError, ValueError, IndexError):
        return ""
def tty(pid):
    t = out("ps", "-p", str(pid), "-o", "tty=")
    return "/dev/" + t if t.strip("?") else ""
mine = os.environ.get("TMUX")
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
        pass
for path in sorted(glob.glob(os.path.expanduser("~/.claude/sessions/*.json"))):
    try:
        with open(path) as f:
            r = json.load(f)
        pid = int(r["pid"])
    except (OSError, ValueError, KeyError, TypeError):
        continue
    start = started(pid)
    alive = bool(start) and start == " ".join(str(r.get("procStart")).split())
    tmux = r.get("tmux") or ""
    pane = tmux.rpartition(".")[2] if "%" in tmux else ""
    if alive and pane and mine:
        here = tty(pid)
        if not here or here != out("tmux", "display", "-p", "-t", pane, "#{pane_tty}"):
            pane = ""
    if alive and cmux:
        pane = "" if tmux else surfaces.get(pid)
    cols = [r.get("name"), pane, r.get("status"), r.get("cwd"), r.get("sessionId")]
    print("\t".join(["alive" if alive else "dead"] + [str(c or "-") for c in cols]))
PY
```

- **Read only `alive` rows.** A `dead` row is a record a process left behind — its pid is gone,
  or now belongs to another process (the start time differs)
- `name` is the `ListAgents` name, `pane` the surface UUID that `--surface` takes. cmux itself
  answers which processes run in which surface (`cmux top`). It is `-` when that session is not
  in a cmux tab of yours — another terminal, or tmux, tmux inside a cmux tab included — and then
  this skill has nothing for that worker
- `cwd` is where the session stands: the root, or one of the worktrees `git worktree list`
  names — wherever they stand. A row standing elsewhere is not a worker of this repository, whatever its name
- `status` is `idle`, `busy` or another word. It has to agree with `ListAgents` where a step
  below asks for `idle`
- Your own row is the one whose pane is `$CMUX_SURFACE_ID`

Run it when a step below needs a surface, once.

## Is the input box empty

    cmux rpc surface.input_state '{"surface_id":"<surface>"}'

cmux reads Claude Code's input box off that surface's screen. The box is **empty** only when
the answer reads `"state": "empty"` and `"waiting_on_human": false`. `draft` is text someone
typed or pasted — the person's, even a half-typed word. `dialog` is a prompt or a menu standing
where the box was. `unknown` is a screen cmux cannot read as Claude Code's — a shell prompt, a
process that ended. `waiting_on_human` is a permission prompt or a question waiting for the
person. None of these is a box to type into, and neither is an error or an answer without
`state`.

**Then do not type. Tell the person which tab holds what, and go on as if this skill were not
here.**
