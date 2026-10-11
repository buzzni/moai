---
name: moai-cmux
description: Use in Claude Code together with moai-supervise when the supervisor runs inside cmux (CMUX_SURFACE_ID is set and $TMUX is not) — to find a worker's surface from its ListAgents name, label its tab, clear a reported worker's window, deliver a message SendMessage could not, read why a worker stalled, and open new worker splits once the person says yes. Triggers on "worker surface", "worker tab", "open a worker", "clear the worker", "일꾼 탭", "일꾼 창 열어", "탭 비워".
---

# moai-cmux — the supervisor's hands on the workers' surfaces

This is the supervisor's (`moai-supervise`) companion **when it runs inside cmux** —
`CMUX_SURFACE_ID` is set in its shell and `$TMUX` is not. Inside tmux — tmux running in a cmux
tab included — what you type goes to tmux: that is `moai-tmux`, and none of this applies.
Outside both tmux and cmux none of this applies either: the supervisor works through messages
and the person alone. The cmux app here is **the person's own**: every workspace they have lives
in it, and every surface you touch is a tab they are looking at.

cmux's words: a window holds workspaces (the rows of its sidebar), a workspace holds panes (its
splits), and a pane holds surfaces (the tabs of that split). A surface is one terminal — what you
read and what you type into.

## First, the version

This needs cmux 0.65.0 or later. Look once, at the start of the round:

    cmux capabilities

If its `methods` do not list `surface.input_state`, the cmux here is older: tell the person once
that `moai-cmux` needs cmux 0.65.0 or later, and go on as if this skill were not here. Updating
restarts cmux, and with it every session inside — yours too — so when is theirs to choose. If it
prints an error instead of a `methods` list — a socket it cannot reach, a sandbox — that says
nothing about the version: tell the person what it said, and go on as if this skill were not here.

## What never happens

- **No headless run.** A worker you open is an ordinary interactive `claude` the person can
  see and type into. Never `claude -p`, never a `--dangerously-*` flag, never
  `--permission-mode`. The moai binary launches and drives nothing; this skill is you
  typing cmux commands where the person can watch
- **Nothing is closed.** Never `close-surface`, `close-workspace` or `close-window`, never
  `respawn-pane`, never `pkill` or `killall` aimed at cmux. A worker that is done stays open;
  closing a tab is the person's
- **Never `--force`.** cmux refuses to type into a Claude Code box that holds someone's words or a
  dialog — but only for a session its own Claude hook knows, which is why you read the box
  yourself first — and `--force` skips that refusal. A refusal is an answer — tell the person
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
import glob, json, os, subprocess, sys
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
    tmux = r.get("tmux") or ""
    pane = tmux.rpartition(".")[2] if "%" in tmux else ""
    if alive and (mine or cmux) and r.get("kind") not in (None, "interactive"):
        pane = ""
    elif alive and pane and mine:
        here = tty(pid)
        if not here or here != device(out("tmux", "display", "-p", "-t", pane, "#{pane_tty}")):
            pane = ""
    elif alive and cmux:
        pane = "" if tmux else surfaces.get(pid)
    rows.append((alive, pane, r))
if rows and not any(alive for alive, _, _ in rows):
    sys.exit("no record reads alive, not even this session's own: this shell cannot read process start times (a sandbox, or no ps), so act on no row")
claimed = [pane for alive, pane, _ in rows if alive and pane and (mine or cmux)]
for alive, pane, r in rows:
    if alive and claimed.count(pane) > 1:
        pane = ""
    cols = [r.get("name"), pane, r.get("status"), r.get("cwd"), r.get("sessionId")]
    print("\t".join(["alive" if alive else "dead"] + [str(c or "-") for c in cols]))
PY
```

- **Read only `alive` rows.** A `dead` row is a record a process left behind — its pid is gone,
  or now belongs to another process (the start time differs)
- `name` is the `ListAgents` name, `pane` the surface UUID that `--surface` takes. cmux itself
  answers which processes run in which surface (`cmux top`). It is `-` when that session is not
  in a cmux tab of yours — another terminal, or tmux, tmux inside a cmux tab included — when it
  shares its tab with another live session, or when cmux gave no answer (a line on its own says
  so), and then this skill has nothing for that worker
- **No row at all, and a line saying no record reads alive:** this shell cannot tell live from
  dead — act on no row, and tell the person what it said
- `cwd` is where the session stands: the root, or one of the worktrees `git worktree list`
  names — wherever they stand. A row standing elsewhere is not a worker of this repository, whatever its name
- `status` is `idle`, `busy` or another word. It has to agree with `ListAgents` where a step
  below asks for `idle`
- Your own row is the one whose pane is `$CMUX_SURFACE_ID`

Run it when a step below needs a surface, once.

## Is the input box empty

    cmux rpc surface.input_state '{"surface_id":"<surface>"}'

cmux reads Claude Code's input box off that surface's screen. The box is **empty** only when
the answer's `state` is `empty` and its `waiting_on_human` is `false`, and the session map, run
again just before, still reads that session `alive` in that tab — cmux reads the screen alone,
and a tab whose Claude Code has ended can still show its last box, and read `empty`, while a
shell has the keyboard. `draft` is text someone typed or pasted — the person's, even a half-typed
word. `dialog` is a prompt or a menu standing where the box was. `unknown` is a screen cmux
cannot read as Claude Code's — a shell prompt, a process that ended. `waiting_on_human` is cmux's
note that the session last asked the person something — a permission prompt, a question — and
it can stay `true` after an API error or an interrupt. None of these is a box to type into, and
neither is an error or an answer without `state`.

**Then do not type. Tell the person which tab holds what, and go on as if this skill were not
here.**

## Label the tab

When you send work (the supervisor's 3), you may write the worker and the work onto its tab.
**Ask the person once per round** whether worker tabs should carry labels — unlike a pane option
in tmux, a label takes the tab's title where they look. On a yes:

    cmux rpc tab.action '{"surface_id":"<surface>","action":"rename","title":"<worker> <id>"}'

`<id>` is the backlog or epic you sent. Go through `rpc`, not `rename-tab`: the CLI's tab words
add your own workspace to the call, and cmux then cannot find a worker tab that stands in
another workspace. A name given this way stays over the title Claude Code keeps writing
(`✳ <topic>`) until it is taken off. Take it off when that work's report is checked (the
supervisor's 5):

    cmux rpc tab.action '{"surface_id":"<surface>","action":"clear_name"}'

**A tab the person named stays theirs.** Read its title before you label it and again before you
take the label off — `cmux --id-format both tree --all` prints each surface's title, in quotes,
beside its UUID. Claude Code's own titles open with a status glyph: `✳`, or a spinner frame such
as `◑` or `⠂`; a label reads `<worker> <id>` — that tab's worker and a backlog or epic id, yours
or one a supervisor before you left. Any other title may be a name the person gave the tab, and
`clear_name` would erase it — do not label that tab, and do not take off a name the person gave it
after your label. Rename no workspace: the names in the sidebar are the person's.

## Clear a worker's window

Before you send a worker its next work, you may clear its window (`/clear`) yourself — all of
these first:

1. **Its report is checked** — the supervisor's 5 held all three checks and the
   `Report-checked:` note is written
2. **It reads `idle`** — in `ListAgents` and in its row of the session map
3. **Its input box is empty** (above)

Then:

    cmux send --surface <surface> -- '/clear\n'

To `send`, `\n` is the Enter key: this types `/clear` and presses it, in one call. Then, as a
separate call, look once: the session map reads `idle` for it and

    cmux read-screen --surface <surface> --lines 20

shows the cleared screen — the conversation gone, an empty box. Then send with `SendMessage`
as the supervisor's 3 says. **If any condition fails, cmux refuses to type, or the look does not
show it cleared, do not type again** — do what the supervisor's 5 says without cmux: ask the
person, or send to another idle worker. When it was the look that failed, tell the person that
`/clear` may stand typed in that tab's box: pressed later, it would erase the next message
sent there.

## When a message does not arrive

When `SendMessage` to a worker fails — an error, no such session — and that worker has a surface
whose input box is empty (above), you may put the message into the box yourself. **A message
held for the person's approval is not one that failed:** that hold is the person's gate, like
a permission prompt, and the held message still arrives once they approve it — pasting it too
skips their gate and hands the worker the same work twice. Tell the person it waits for them
instead. Typed keys submit at every newline, so paste it as one block:

1. Write the message to a file in your scratchpad, with one line at the end naming you — a
   pasted message carries no sender, and the worker reports to the message's `from`. The
   first line stays the message's own, which names the work and the step to start from:
   `from: <your ListAgents name>`
2. Paste and submit it — the lone `-` reads the file from stdin; after a `--` it would be pasted
   as the text itself:

       cmux paste --surface <surface> --submit - < <file>

   It goes in as one paste, and cmux presses the key that submits it. If cmux refuses —
   someone's words or a dialog in the box — do not try again. If it warns that the text was
   pasted but the submit key was not sent, do not paste again either: the message stands in
   that box unsent
3. **Tell the person** you did, and into which tab — and, after that warning, that the message
   waits there unsent

The worker still answers with `SendMessage`. A worker that answered your message by refusing
the work is not a delivery that failed — it comes out of the candidates (the supervisor's 2).

## When a worker stalls

When a `notify_when_idle` notice comes with no report, or the person asks about a worker that
has read `busy` far longer than its work should take, look at its tab once:

    cmux read-screen --surface <surface> --lines 40

and read why it stopped — a permission prompt, a question (`AskUserQuestion`), an API or
rate-limit error, or a process that ended (a shell prompt where the box was, or under its last
box). The input box check above gives a hint when you have it: `waiting_on_human` says the
session last asked the person something — it can outlast an API error, so the screen decides —
and `unknown` is a screen that is no longer Claude Code's; whether the session is still alive,
the session map says. **Tell the person** what the tab
shows and which tab it is. Do not answer the prompt, do not press a key, and do not look again
in a loop. A process that ended is a dead session: bringing it back is `moai-recover`, once
the person asks for it.

## No idle worker left

When the supervisor's 2 finds no worker, ask the person **once** whether to open new worker
tabs, and how many — one question covers several. Open one only on their yes, or when they
asked you for it. On a yes, for each:

    cmux --id-format both new-split right --surface "$CMUX_SURFACE_ID" --command 'cd <root> && claude --model <model>'

`<root>` is the `root dir` of the supervisor's 2 and `<model>` the model picked in 2-1. **No
other flag** on `claude`. It prints `OK surface:<n> (<UUID>) workspace:<n> (<UUID>)` — the first
UUID is the new surface, the second your workspace. `--command` is typed into the new tab's
shell, so the shell stays when `claude` exits and what it said stays readable, and the split
does not take focus. **If cmux refuses the split** — no space for a new pane, as every split
lands in your row, or it cannot find your tab in the workspace this session started in — open
no more: tell the person how many opened and what it said. When all are open, even the splits
out once:

    cmux rpc workspace.equalize_splits '{"workspace_id":"<workspace UUID>"}'

The tab belongs to the same person — it is their interactive session like any other.

Once it has started (a separate call: the input box check above reads `empty` for the new
surface), look at `ListAgents` once more. The new row is a worker like any other (the
supervisor's 2) — send to it as in 3. If it does not show yet, look once more after your next
step; if a prompt stands in the new tab (trusting the folder, say), tell the person — it is
theirs to answer.
