---
name: moai-recover
description: Use in Claude Code when the person asks to bring back the sessions of this repository that died — after a restart, an OOM kill or a crash — so the supervisor and its workers carry on where they stopped. Finds the dead sessions, draws the state each was in, points out background work that never returned and broken target links, and resumes each one in a new tmux pane, or prints the command to type. Triggers on "recover the sessions", "bring the sessions back", "resume the dead sessions", "되살려", "세션 복구", "이어 가게 해".
---

# moai-recover — bring back the sessions that died

Use this when the person asks for it: the sessions of this repository — a supervisor, its
workers — died together, after a restart, an OOM kill or a crash, and the person wants them
to carry on. **Their asking is the yes:** inside tmux you open the panes without asking again.
None of this is a moai command; it is you reading Claude Code's own records and typing where
the person can watch.

## What never happens

- **No headless run.** A session you bring back is an ordinary interactive `claude --resume`
  the person can see and type into. Never `claude -p`, never a `--dangerously-*` flag, never
  `--permission-mode`
- **Nothing is killed.** Never `kill-server`, `kill-session` or `kill-pane`, never `pkill`
  or `killall` aimed at tmux (hook rule 4). A session that is alive is left alone
- **Only the panes you opened.** Type only into a pane this skill opened, by the `%N` that
  `split-window` printed — every call names it, `-t <pane>`
- **Never over the person's words.** Paste only into an empty input box ("Is the input box
  empty" below); otherwise tell the person
- **No polling.** Every look is one look; the next comes after your next step, as its own
  call — never a loop, never a `sleep`
- **Their work stays as it is.** No commit, no checkout, no stash, no build, no `moai` write —
  what a dead session left is for that session to pick up

## Is the input box empty

    tmux display -p -t <pane> '#{pane_in_mode}'
    tmux capture-pane -p -e -t <pane>

The first line prints `1` while the person is scrolling the pane (copy mode) — keys you send
then go to tmux's copy mode, not to Claude Code, so a pane in a mode is theirs: do not type.
Claude Code's input box is the line that starts with `❯`, under the conversation, between two
`─` rules. **Empty** is `❯` followed by nothing, or by Claude Code's dim placeholder — `-e`
keeps the colours, and the placeholder is drawn dim (SGR `2`, or a grey foreground) where the
person's text is not. Anything else — a word, a pasted block, a half-typed command — is the
person's, and if you cannot tell the placeholder from their draft, it is theirs. A pane with
no `❯` box at all (a shell prompt, a dialog) is not a box to type into either.

**When it is not empty, do not paste.** Tell the person which pane holds what — the words in it
are theirs.

## 1. Find the dead

Claude Code keeps one record per process under `~/.claude/sessions/`, and a process that dies
leaves its record behind. The lines below print one row per record, tab-separated:

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

- A **candidate** is a `dead` row with a `sessionId` whose `cwd` is the root or one of the
  worktrees `git worktree list` names
- **Drop it when it is back already** — a live row carries the same `sessionId` (`--resume`
  keeps the id), **your own row included**: a session the person resumed first and then asked
  for this is back already, and resuming it again puts one conversation in two windows. In a
  worktree, also drop it when a live row other than your own stands in that same `cwd`. The
  root is not such a place — several live sessions stand there at once, and one of them being
  alive says nothing about the dead one
- **Drop old crashes.** Records of earlier deaths stay too. Keep the candidates whose
  transcript (below) last moved around the same time; name an older one to the person apart,
  and bring it back only if they say so
- When several candidates stand in the same `cwd`, ask the person **once** — one question
  for all of them — which to bring back

**A session's transcript** is `~/.claude/projects/<slug>/<sessionId>.jsonl`, one JSON object
per line. The slug is a directory with every character that is not a letter or a digit
turned into `-` (`/home/me/repo/.worktrees/moai-ab12` is `-home-me-repo--worktrees-moai-ab12`).
Which directory is not fixed: a worker that entered its worktree from the root has kept its
transcript under the root's slug on one Claude Code version and under the worktree's on
another, so do not work the slug out — find it by id:

    ls ~/.claude/projects/*/<sessionId>.jsonl

**When the records are gone too**, look under the root's slug and under each worktree's slug
for every `*.jsonl` that last moved around the time they died — the root's slug may hold the
supervisor's and a worker's both, so not only the newest one. A line's `cwd` and `sessionId`
say where that session last stood and which id to resume; its `timestamp` says when it last
moved.

## 2. Draw the state each was in

Read each candidate's transcript from the end — the last user line and the last assistant
line — and look at where it stood:

- **Role.** The supervisor's transcript runs `moai-supervise`; a worker's holds the
  supervisor's message naming `references/worker.md`. A session that is neither is brought
  back the same way, as a worker
- **Work.** The issue or epic id it held, and what it was doing or waiting for — a report,
  a review, a person's decision, a build
- **Died at** — the `timestamp` of its last line
- `git -C <cwd> status --short` — what it left uncommitted
- `moai show <id>` — a `Next:` note on that id says where the session meant to go on

Show the person **one table**, a row per session: role, name, `cwd`, work id, what it was
waiting for, died at, uncommitted files. Then go on — they asked for recovery already.

## 3. Point out what died with it

- **Background work that never returned.** A background launch is an `Agent` or `Bash`
  `tool_use` whose result reads `Async agent launched` (with its `agentId`), `Command running
  in background with ID: <id>`, or — a command that ran past its timeout —
  `moved to the background (ID: <id>)`. Whether the input carries
  `"run_in_background": true` differs by version and the timeout case carries nothing, so read
  the result; when it ends, a later user line carries a
  `<task-notification>` whose `<task-id>` is that id. A launch with no such line after it died
  with the session — the resumed session will never hear from it, and what it
  changed is uncommitted in that session's `cwd`
- **A broken target link.** In the root and each worktree, `target` may be a link to
  `/tmp/cargo-target/<name>`, and a restart can empty `/tmp`:

      readlink <cwd>/target
      test -e <cwd>/target || echo dangling

  Make the directory again, `mkdir -p /tmp/cargo-target/<name>`, so builds land there. The
  build output is gone: `./target/release/moai` needs `cargo build --release` before anything
  calls it. Do not build it yourself — the resumed session does

## 4. The text each one gets

One block per session, written to a file in your scratchpad:

    Recovery: this session died at <time> (<what happened>) and was resumed.
    - Background work that never returned: <each launch and what it was for>. It is dead,
      do not wait for it; what it changed is uncommitted in <cwd>.
    - git status --short in <cwd>: <files, or clean>.
    - target -> /tmp/cargo-target/<name> was gone and is made again; build before you
      call ./target/release/moai.
    Go on from where you stopped: <work id>, <what it was waiting for>.

Leave out a line that does not hold. The supervisor's block adds one line: **the workers are
back in new sessions and their names may have changed — run `ListAgents` again** before you
send or wait for a report.

**Workers first, the supervisor last**, in both ways below — the supervisor's `ListAgents`
has to see the workers when it starts.

## 5. Inside tmux — open a pane each

`$TMUX` is set in your shell. For each session, in that order:

    tmux split-window -P -F '#{pane_id}' -t "$TMUX_PANE" -c <cwd> 'claude --resume <sessionId>; exec bash'
    tmux select-layout -t "$TMUX_PANE" tiled

**No other flag.** The first line prints the new pane's `%N`; `exec bash` keeps a shell there
when `claude` exits. Then, as a separate call, look once: when its `❯` box shows and is empty
("Is the input box empty" above), paste the block:

    tmux load-buffer -b moai-recover <file>
    tmux paste-buffer -p -d -b moai-recover -t <pane>
    tmux send-keys -t <pane> Enter

If the box has not shown yet, look again after your next step. A prompt in the pane (trusting
the folder, say) is the person's to answer — tell them which pane. When all are open, tell the
person which pane is which session.

## 6. Outside tmux — say what to type

`$TMUX` is not set: open nothing. Print, per session in the same order, the line the person
types in a terminal of their own, and under it the block to paste once its box shows:

    cd <cwd> && claude --resume <sessionId>
