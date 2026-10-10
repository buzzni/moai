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
through this skill. `moai-tmux`, where it is planted, is for a worker Saycode does not list,
and for what Saycode cannot do — the pane labels.

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

The supervisor's 2 picks workers from `ListAgents`, and a worker reports with `SendMessage`,
so one worker stands in both lists. Claude Code keeps one record per process under
`~/.claude/sessions/`, and the lines below pair them — `saycode` is the Saycode id read from
that process's environment (`APLUS_SESSION_ID`), tab-separated:

    state  name  pane  status  cwd  sessionId  saycode

```sh
python3 - <<'PY'
import glob, json, os
mine = os.environ.get("TMUX", "").split(",")[0]
rows = []
for path in sorted(glob.glob(os.path.expanduser("~/.claude/sessions/*.json"))):
    try:
        with open(path) as f:
            r = json.load(f)
        pid = int(r["pid"])
    except (OSError, ValueError, KeyError, TypeError):
        continue
    try:
        with open(f"/proc/{pid}/stat") as f:
            stat = f.read()
        alive = stat[stat.rindex(")") + 2:].split()[19] == str(r.get("procStart"))
    except (OSError, ValueError, IndexError):
        alive = False
    env = {}
    if alive:
        try:
            with open(f"/proc/{pid}/environ", "rb") as f:
                env = dict(v.split(b"=", 1) for v in f.read().split(b"\0") if b"=" in v)
        except OSError:
            pass
    tmux = r.get("tmux") or ""
    pane = tmux.rpartition(".")[2] if "%" in tmux else ""
    if alive and pane and mine and env.get(b"TMUX", b"").split(b",")[0].decode(errors="replace") != mine:
        pane = ""
    saycode = env.get(b"APLUS_SESSION_ID", b"").decode(errors="replace")
    rows.append([alive, r.get("name"), pane, r.get("status"), r.get("cwd"), r.get("sessionId"), saycode])
panes = [row[2] for row in rows if row[0] and row[2]]
for row in rows:
    if row[0] and row[2] and panes.count(row[2]) > 1:
        row[2] = ""
    print("\t".join(["alive" if row[0] else "dead"] + [str(c or "-") for c in row[1:]]))
PY
```

- Read only `alive` rows. `name` is the `ListAgents` name and `saycode` the Saycode id, `-`
  when that process is not a Saycode session
- A `ListAgents` row and a Saycode row the map pairs are **one worker** — count it once
- A worker `ListAgents` shows whose `saycode` is `-` is not a Saycode session: it goes
  through `moai-tmux` where that stands, otherwise through messages and the person, as the
  supervisor's steps say
- A Saycode row the map pairs with nothing is not Claude Code — a `codex` or `gemini` worker
  the person asked for. It cannot `SendMessage`; its report is its last turn ("Take the
  report")
- **Never trust `pane` for a Saycode row.** Saycode's daemon hands its own pane down to the
  sessions it starts, so the record names a pane that is not theirs — the map prints `-` for
  a pane two alive rows name

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
   a short turn and block until its timeout
4. Send it, without `--wait`:

       happy agent prompt <id> "$(cat <file>)"

   It answers `"ok":true` with `delivered: true` once Saycode took it. On `"ok":false` it did
   not go — do not send it again blind; tell the person

The `Sent:` note of the supervisor's 3 names the worker as `<name> (saycode <id>)`, so a
supervisor that starts again finds it after a `/clear` renamed it. Leave `notify_when_idle`
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

and, as a separate call, look once with `ls --status`: it reads `idle`. The context is gone
and its process started again, so its `ListAgents` name has changed — run `ListAgents` and
the map again before anything goes to it by name. Its Saycode id is the same, and "Send work"
needs no name. **If any condition fails, do not clear it** — do what the supervisor's 5 says:
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

To be told when a worker starts waiting, without polling, start in the background:

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
`claude` with the model picked in the supervisor's 2-1.

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

A new Claude Code worker shows in `ListAgents` too, and the map pairs it. A worker that is not
Claude Code reports through its last turn ("Take the report").
