---
name: moai-saycode
description: Use in Claude Code together with moai-supervise when the supervisor runs inside a Saycode session (SAYCODE_AGENT_ENV is set) — to list the workers' Saycode sessions and their state, send a worker its work and be told when its turn ends, read what it said, clear a reported worker, find a worker stalled on a question or a permission, and open new worker sessions once the person says yes. Triggers on "saycode", "worker session", "open a worker", "일꾼 세션", "일꾼 열어", "세이코드".
---

# moai-saycode — the supervisor's hands on the workers' Saycode sessions

This is the supervisor's (`moai-supervise`) companion **when it runs inside a Saycode
session** — `SAYCODE_AGENT_ENV` is `1` in its shell. Saycode runs each session under its
daemon and lets a session see and drive the others in the same scope directory. The binary
on `PATH` is `happy` (its help calls itself `saycode`). Every `happy agent` verb prints one
JSON line on stdout; on failure it exits 3 with `{"ok":false,"error":…}`.

Once, before the round:

    happy agent whoami

If it does not answer `"ok":true`, this skill has nothing here — go on as if it were not
loaded. Its `sessionId` is your own Saycode id, and `spawnsRemaining` and
`spawnModelOptions` are for "No idle worker left".

**Saycode first, tmux second** (the person's decision). A Saycode session often runs inside
tmux too, so `$TMUX` stands as well. A worker that `happy agent ls --status` lists is driven
through this skill. `moai-tmux` (or `moai-cmux` in a cmux tab), where it is planted, is for a
worker Saycode does not list, and for what Saycode cannot do — the pane labels.

## What never happens

- **No headless run.** A worker is an interactive session the person can open in Saycode.
  Never `claude -p`, never `--permission-mode`. The moai binary launches and drives nothing;
  this skill is you typing `happy agent` verbs the person can see in your conversation
- **Never `steer` or `stop` a worker** unless the person asks for it. A response that runs is
  theirs to stop
- **Only a worker of this repository.** Never a session whose `directory` lies outside it, and
  never your own
- **Never a second turn behind a running one.** Do not `prompt` a session whose `state` is
  `responding` or `waiting-input` — the prompt queues as its next turn and runs after what
  runs now, after the person's answer if it waits on one
- **Never in their place.** A pending question, plan approval or permission request is the
  person's to answer — you read it and tell them
- **Never `prompt --wait`.** It fails with `prompt_stalled` after five seconds though the
  message arrived, and a second send runs it twice
- **No polling.** Every look below is one look. What you wait for, a background
  `happy agent wait` tells you — never a loop and never a `sleep`
- **A new session only after the person's yes** ("No idle worker left")

## Which session is which worker

    happy agent ls --status

It answers `{"sessions":[…],"ok":true}`, one object per session in Saycode's scope: `id`,
`directory`, `summary`, `state` (`idle`, `responding`, `waiting-input` or `ended`),
`pending` (`askUserQuestion`, `exitPlanMode`, and `permissionRequests` — a count),
`lastAgentText`, `turnEndedAt` and `lastSeq`.

- A worker's row is one whose `directory` is the root or one of the worktrees
  `git worktree list` names. Any other directory is not this repository's
- Skip your own (`whoami`'s `sessionId`) and every `ended` row. An ended session is dead —
  bringing it back is `moai-recover`, once the person asks
- **The Saycode id is the key.** `/clear` restarts a worker's process under the same Saycode
  session: its `ListAgents` name changes, its Saycode id does not. Keep your book of what
  you sent by Saycode id
- `lastSeq` is the cursor of its messages ("Take the report")

The supervisor's 2 picks workers from `ListAgents`, and a Claude Code worker Saycode runs shows
there too. **Nothing pairs a `ListAgents` name with a Saycode id**, so the two lists are kept
apart: a worker driven through Saycode is a row of `happy agent ls --status`, and the
`ListAgents` candidates the supervisor's 2 counts are only those the map below does not mark
`saycode`. The two sets do not overlap, so no worker is counted twice. Claude Code keeps one
record per process under `~/.claude/sessions/`, and the map reads them — `saycode` is the
record's own marker: a session Saycode started (in its app, or by its daemon) carries the
entrypoint `remote_mobile`, a terminal session `cli`. Tab-separated:

    state  name  pane  status  cwd  sessionId  saycode

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
    if r.get("entrypoint") == "remote_mobile":
        pane = ""
    elif alive and (mine or cmux) and r.get("kind") not in (None, "interactive"):
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
    saycode = "saycode" if r.get("entrypoint") == "remote_mobile" else ""
    cols = [r.get("name"), pane, r.get("status"), r.get("cwd"), r.get("sessionId"), saycode]
    print("\t".join(["alive" if alive else "dead"] + [str(c or "-") for c in cols]))
PY
```

- Read only `alive` rows. `name` is the `ListAgents` name; `saycode` reads `saycode` for a
  session Saycode started and `-` for any other
- A `ListAgents` row the map marks `saycode` is **left out of the supervisor's 2** — that
  worker is counted, sent to and cleared here, by its Saycode id
- A `ListAgents` row whose `saycode` is `-` is not a Saycode session: it goes through
  `moai-tmux` (or `moai-cmux`) where that stands, otherwise through messages and the person,
  as the supervisor's steps say
- **Every Saycode row of this repository is a worker** — a candidate while it reads `idle`
  and the report of the work you last sent it is checked. It may be a `codex` or `gemini`
  worker the person asked for, which `ListAgents` never shows. Its `Sent:` note names it
  `saycode <id>`
- **A report is tied to the work, not to a name.** A Claude Code worker reports with
  `SendMessage`, and its `from:` is a `ListAgents` name nothing pairs with an id — the
  report's `report: <epic>` and the epic's `Sent:` note (`saycode <id>`) say which worker it
  was. A worker that is not Claude Code cannot `SendMessage`; its report is its last turn
  ("Take the report")
- **Never trust `pane` for a Saycode row.** Saycode's daemon hands its own pane down to the
  sessions it starts, so the record names a pane that is not theirs — the map prints `-` for
  a session the daemon started, and, inside tmux or cmux, for a pane two alive rows name

Run each of these when a step below needs it, once.

## Send work

The supervisor's 3, for a worker Saycode lists. Send only to a row that reads `state: idle`
with `askUserQuestion` and `exitPlanMode` false and `permissionRequests` 0.

1. Write the message of the supervisor's 3 to a file in your scratchpad, with one line at
   the end naming you — a prompt carries no sender, and the worker reports to the message's
   `from`. The first line stays the message's own, which names the work and the step to
   start from: `from: <your ListAgents name>`
2. Note the worker's `lastSeq` from `ls --status`
3. Start the wait **in the background** (Bash `run_in_background`), before you send:

       happy agent wait <id> --until turn-end --timeout 3600000

   `wait` counts only a turn that ends after it started. Started after the prompt, it can miss
   a short turn and block until its timeout. Give that Bash call a `timeout` above the wait's
   own, `3660000` — Bash stops a background command after 30 minutes unless told otherwise,
   and a stopped wait is neither of the two endings below
4. Send it, without `--wait`:

       happy agent prompt <id> "$(cat <file>)"

   It answers `"ok":true` with `delivered: true` once Saycode took it. On `"ok":false` it did
   not go — do not send it again blind; tell the person

The `Sent:` note of the supervisor's 3 names the worker as `saycode <id>` — a `/clear` renames
it in `ListAgents` but not here, so a supervisor that starts again finds it. Leave `notify_when_idle`
out: the background wait ending is the notice that the turn ended. It ends `satisfied` when
the turn ended, or `wait_timeout` after an hour — then look once with `ls --status`, and start
another wait only while the row still reads `responding`.

## Take the report

The report still comes by `SendMessage`, `report: <epic>` at its head, as the supervisor's 4
says. When the wait ended and no report came, read what the worker said, once:

    happy agent read <id> --since <the lastSeq you noted>

It holds the worker's messages only — not your prompt, and a step that only ran tools has
empty text. A worker that asked its person something is in "When a worker stalls". A worker
that is not Claude Code cannot `SendMessage`: its report is this last turn — check it as the
supervisor's 5 says.

## Clear a worker

Before you send a worker its next work, you may clear it yourself — all of these first:

1. **Its report is checked** — the supervisor's 5 held all three checks and the
   `Report-checked:` note is written
2. **Its row reads `idle`** in `ls --status`, with `askUserQuestion` and `exitPlanMode` false
   and `permissionRequests` 0
3. **The person is not talking to it.** You cannot see the box where they type in Saycode —
   if they said they are working with that worker, do not clear it

Then:

    happy agent prompt <id> /clear

and, as a separate call, look once with `ls --status`: it should read `idle`. Until it does,
send it nothing — look again after your next step. The context is gone
and its process started again, so its `ListAgents` name has changed; its Saycode id is the
same, and "Send work" needs no name. **If any condition fails, do not clear it** — do what the supervisor's 5 says:
ask the person, or send to another idle worker.

## When a worker stalls

When the wait ends with no report, or the person asks about a worker, look at its row in
`ls --status` once:

- **`waiting-input`** — `askUserQuestion` or `exitPlanMode` is true, or `permissionRequests`
  is above 0: the worker waits on its person. Read the question once — `lastAgentText`, or

      happy agent read <id> --last 3

  and **tell the person** which session (its id and `summary`) waits for what. Do not answer it
  and do not prompt that session
- **`ended`** while your work was out — the session died. Bringing it back is
  `moai-recover`, once the person asks
- **`responding`** far longer than the work should take — read its last messages once,
  `happy agent read <id> --last 5`, and tell the person what they show

To be told when a worker starts waiting, without polling, start in the background, with the
same Bash `timeout` as in "Send work":

    happy agent wait <id> --until waiting-input --timeout 3600000

## No idle worker left

When the supervisor's 2 finds no worker, ask the person **once** whether to open new worker
sessions, and how many — one question covers several. Open one only on their yes, or when
they asked you for it. The question says, for each session:

- it opens as a Saycode session in the root and shows in Saycode's session list — not under
  this conversation
- **it runs with permission prompts bypassed.** Saycode starts it with
  `--dangerously-skip-permissions`, and spawn sends no permission mode. The person allowed
  that on 2026-10-10; say it every time anyway
- it uses one of your spawn budget — `whoami`'s `spawnsRemaining`, failed attempts included

**Agent, model and effort are the person's.** They may name a worker that is not Claude Code
(`codex`, `gemini`). Take exactly what they name, check it against `whoami`'s
`spawnModelOptions` (each `agent` with its `models`, each model with the `efforts` it takes),
and never fill in a value they left out — leave that flag off. When they name nothing, it is
`claude` with the model picked in the supervisor's 2-1. Spawn takes no alias — `--model opus`
fails as an unknown model — so `<model>` is the first id under `claude` in `spawnModelOptions`
that carries that word (`claude-opus-…` for `opus`); when none does, leave `--model` off.

On a yes, for each session:

    happy agent spawn --prompt "$(cat <file>)" --agent <agent> --model <model> --effort <effort>

`<file>` is the message of "Send work" with its `from:` line — the first prompt **is** the
assignment. Drop `--effort` when no effort was named. **No other flag.** Saycode's own
worktree lands in `.aplus/worktrees/<random name>`, not where the worker's steps put it; without
one the session starts in the root and makes its worktree itself. Right after it answers with
the new session's `sessionId`, start the background wait of "Send work" on it — its first turn ends on
its own.

- `invalid_spawn_options` — ask the person again. Never fall back to another agent or model
- `spawn_limit_exceeded` — the budget is spent; tell the person, with `whoami`'s
  `budgetResetsAt`
- `spawn_depth_exceeded` — this session cannot open sessions; tell the person

A new Claude Code worker shows in `ListAgents` too, and the map marks it `saycode`, so the
supervisor's 2 leaves it out there. A worker that is not Claude Code reports through its last
turn ("Take the report").
