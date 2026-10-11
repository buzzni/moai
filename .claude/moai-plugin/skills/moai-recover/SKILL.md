---
name: moai-recover
description: Use in Claude Code when the person asks to bring back the sessions of this repository that died — after a restart, an OOM kill or a crash — so the supervisor and its workers carry on where they stopped. Finds the dead sessions, draws the state each was in, points out background work that never returned and broken target links, and resumes each one in a new tmux pane or cmux tab, or prints the command to type. Triggers on "recover the sessions", "bring the sessions back", "resume the dead sessions", "되살려", "세션 복구", "이어 가게 해".
---

# moai-recover — bring back the sessions that died

Use this when the person asks for it: the sessions of this repository — a supervisor, its
workers — died together, after a restart, an OOM kill or a crash, and the person wants them
to carry on. **Their asking is the yes:** inside tmux or cmux you open the panes without asking
again.
None of this is a moai command; it is you reading Claude Code's own records and typing where
the person can watch.

## What never happens

- **No headless run.** A session you bring back is an ordinary interactive `claude --resume`
  the person can see and type into. Never `claude -p`, never a `--dangerously-*` flag, never
  `--permission-mode`
- **Nothing is killed.** Never `kill-server`, `kill-session` or `kill-pane`, never `pkill`
  or `killall` aimed at tmux (hook rule 4); inside cmux never `close-surface`, `close-workspace`
  or `close-window`, and never `--force`. A session that is alive is left alone
- **Only the panes you opened.** Type only into a pane this skill opened, by the `%N` that
  `split-window` printed — every call names it, `-t <pane>`. Inside cmux it is the surface UUID
  `new-split` printed, `--surface <surface>`
- **Never over the person's words.** Paste only into an empty input box ("Is the input box
  empty" below); otherwise tell the person
- **No polling.** Every look is one look; the next comes after your next step, as its own
  call — never a loop, never a `sleep`
- **Their work stays as it is.** No commit, no checkout, no stash, no build, no `moai` write —
  what a dead session left is for that session to pick up

## Is the input box empty

Inside tmux:

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

Inside cmux:

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

**When it is not empty, do not paste.** Tell the person which pane holds what — the words in it
are theirs.

## 1. Find the dead

Claude Code keeps one record per process under `~/.claude/sessions/`, and a process that dies
leaves its record behind. The lines below print one row per record, tab-separated:

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

- **No row at all, and a line saying no record reads alive** — not even your own: this shell
  cannot tell live from dead (a sandbox that hides other processes, say). Bring nothing back;
  tell the person what it said
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

**A Saycode session** comes back through Saycode (8), never in a pane — resumed in a pane, its
conversation would run outside Saycode while the Saycode session stays ended. Its transcript
(below) says which candidate is one: its lines carry `"entrypoint":"remote_mobile"`, where a
session opened in a terminal carries `"cli"`. That holds outside Saycode too, and for a
session that died in the middle of a turn.

Its Saycode id comes from Saycode. **Inside Saycode** — `SAYCODE_AGENT_ENV` is `1` in your
shell and `happy agent whoami` answers `"ok":true` — a session Saycode ran leaves a row behind:

    happy agent ls --status

A row whose `state` is `ended` and whose `directory` is the root or one of the worktrees is a
Saycode session that died. Pair it with a Saycode candidate by what it last said: its
`lastAgentText` is the start of the last assistant text in that candidate's transcript (2).
It is empty for a session that died in the middle of a turn — pair that one by its `summary`
against what the transcript was doing, and ask the person when two rows could match. Name an
ended row that pairs with no candidate to the person apart, with its `summary`. A Saycode
candidate left without an id — outside Saycode, or paired with no row — is still never
resumed in a pane: the person reopens it from Saycode's session list (8).

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
waiting for, died at, uncommitted files — and the Saycode id of a Saycode session. Then go on — they asked for recovery already.

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

**Workers first, the supervisor last**, in every way below — the supervisor's `ListAgents`
has to see the workers when it starts. A Saycode session is back only once the person
reopens it (8), so when a worker is one, give the person its line first and open the
supervisor's pane — or print its line — only after they say that worker is back.

## 5. Inside tmux — open a pane each

`$TMUX` is set in your shell. For each session that is not a Saycode session (8), in that
order. The panes open in the window of `$TMUX_PANE` — inside a session Saycode's daemon
started, that is the daemon's pane, not yours — so say which window they are in:

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

## 6. Inside cmux — open a tab each

`CMUX_SURFACE_ID` is set in your shell and `$TMUX` is not — and the cmux is 0.65.0 or later:
`cmux capabilities` lists `surface.input_state` in its `methods`. On an older cmux, tell the
person that this way needs cmux 0.65.0, and go to 7; when it prints an error instead of a
`methods` list, tell them what it said, and go to 7. For each session that is not a Saycode
session (8), in that order:

    cmux --id-format both new-split right --surface "$CMUX_SURFACE_ID" --command 'cd <cwd> && claude --resume <sessionId>'

**No other flag.** It prints `OK surface:<n> (<UUID>) workspace:<n> (<UUID>)` — the first UUID
is the new tab, the second your workspace. The command is typed into the tab's shell, so the
shell stays when `claude` exits, and the split does not take focus. **If cmux refuses a split**
— no space for a new pane, as every split lands in your row, or it cannot find your tab in the
workspace this session started in — open no more: the sessions left, the supervisor among them,
get the lines of 7, and tell the person what it said. When all are open, even the splits out
once:

    cmux rpc workspace.equalize_splits '{"workspace_id":"<workspace UUID>"}'

Then, as a separate call per tab, look once: when its box is empty ("Is the input box empty"
above — the session map reads the resumed session `alive` in that tab), paste the block:

    cmux paste --surface <surface> --submit - < <file>

The lone `-` reads the file from stdin. If cmux refuses — someone's words or a dialog in the
box — do not try again; tell the person. If it warns that the text was pasted but the submit key
was not sent, do not paste again either: tell the person the block stands unsent in that tab's
box. If the box has not shown yet, look again after your next step. A prompt in the tab (trusting
the folder, say) is the person's to answer — tell them which tab. When all are open, tell the
person which tab is which session.

## 7. Outside tmux and cmux — say what to type

Neither way above stands: open nothing. Print, per session in the same order, the line the
person types in a terminal of their own, and under it the block to paste once its box shows:

    cd <cwd> && claude --resume <sessionId>

A Saycode session gets the line of 8 instead.

## 8. Saycode sessions — say how to reopen them

No `happy agent` verb brings an ended session back, and you open none in its place — a new
session from `spawn` is not the one that died. The person reopens each, in the same order as
above: in Saycode's session list, or in a terminal of their own:

    happy resume <saycode id>

It resumes the conversation in the path Saycode saved, **in the foreground of the terminal
that runs it** — so never run it yourself; print it, and under it the block of 4 to paste once
its box shows. A session with no Saycode id (1) has only the session list: name it by its
`cwd`, its `sessionId` and what it was doing. Whether it comes back under the same Saycode id
is not verified: add to the supervisor's block that it reads `happy agent ls --status` again
before it sends.
