---
name: moai-tmux
description: Use in Claude Code together with moai-supervise when the supervisor runs inside tmux ($TMUX is set) — to find a worker's pane from its ListAgents name, label it, clear a reported worker's window, deliver a message SendMessage could not, read why a worker stalled, and open new worker panes once the person says yes. Triggers on "worker pane", "open a worker", "clear the worker", "일꾼 칸", "일꾼 창 열어", "칸 비워".
---

# moai-tmux — the supervisor's hands on the workers' panes

This is the supervisor's (`moai-supervise`) companion **when it runs inside tmux** — `$TMUX`
is set in its shell. Without `$TMUX` none of this applies: the supervisor works through
messages and the person alone. The tmux server here is **the person's own**: every session
they have lives on it, and every pane you touch is one they are looking at.

## What never happens

- **No headless run.** A worker you open is an ordinary interactive `claude` the person can
  see and type into. Never `claude -p`, never a `--dangerously-*` flag, never
  `--permission-mode`. The moai binary launches and drives nothing; this skill is you
  typing tmux commands where the person can watch
- **Nothing is killed.** Never `kill-server`, `kill-session` or `kill-pane`, never `pkill`
  or `killall` aimed at tmux — one bare `kill-server` once took down every session the
  person had (hook rule 4, and the person's own guard hook refuses it too). A worker that
  is done stays open; closing a pane is the person's
- **Only a worker's pane.** Type only into a pane found through the session map below for a
  worker of this repository — never your own pane (`$TMUX_PANE`), never a session of another
  repository, never a test server's pane. Every call names its pane, `-t <pane>`, as `%N`
- **Never over the person's words.** If a worker's input box holds anything, do not type into
  it — tell the person
- **Never in their place.** A permission prompt or a question in a worker's pane is the
  person's to answer — you read it and tell them; you press no key
- **No polling.** Every look below is one look. A second one comes after your next step, as
  its own call — never a loop and never a `sleep`
- **The person's tmux config file stays theirs.** What you set is on the running server

## Which pane is which worker

`ListAgents` names a session; tmux needs a pane. Claude Code keeps one record per process
under `~/.claude/sessions/`, and the lines below print one row per record, tab-separated:

    state  name  pane  status  cwd  sessionId

```sh
python3 - <<'PY'
import glob, json, os
mine = os.environ.get("TMUX", "").split(",")[0]
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
    tmux = r.get("tmux") or ""
    pane = tmux.rpartition(".")[2] if "%" in tmux else ""
    if alive and pane and mine:
        try:
            with open(f"/proc/{pid}/environ", "rb") as f:
                env = dict(v.split(b"=", 1) for v in f.read().split(b"\0") if b"=" in v)
            theirs = env.get(b"TMUX", b"").split(b",")[0].decode(errors="replace")
        except OSError:
            theirs = ""
        if theirs != mine:
            pane = ""
    cols = [r.get("name"), pane, r.get("status"), r.get("cwd"), r.get("sessionId")]
    print("\t".join(["alive" if alive else "dead"] + [str(c or "-") for c in cols]))
PY
```

- **Read only `alive` rows.** A `dead` row is a record a process left behind — its pid is gone,
  or now belongs to another process (the start time differs)
- `name` is the `ListAgents` name, `pane` the `%N` that `-t` takes — `-` when that session
  is not in tmux or runs on another tmux server than yours (pane ids are counted per server,
  so another server's `%4` is a different pane here), and then this skill has nothing for
  that worker
- `cwd` is where the session stands: the root, or one of the worktrees `git worktree list`
  names — wherever they stand. A row standing elsewhere is not a worker of this repository, whatever its name
- `status` is `idle`, `busy` or another word. It has to agree with `ListAgents` where a step
  below asks for `idle`
- Your own row is the one whose pane is `$TMUX_PANE`

Run it when a step below needs a pane, once.

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

**Then do not type. Tell the person which pane holds what, and go on as if this skill were not
here.**

## Label the pane

When you send work (the supervisor's 3), write the worker and the work onto its pane:

    tmux set-option -p -t <pane> @moai '<worker> <id>'

`<id>` is the backlog or epic you sent. Claude Code writes its own topic into the pane title,
so the label lives in the pane option `@moai`. Remove it when that work's report is checked
(the supervisor's 5):

    tmux set-option -p -u -t <pane> @moai

**Showing it is the person's choice, asked once per round.** Ask whether the labels should
show on the pane borders. On a yes, read the running server's two settings:

    tmux show -gv pane-border-format
    tmux show -gv pane-border-status

and put the label in front of the format you read:

    tmux set -g pane-border-format '#{?@moai,#{@moai} ,}<the value you read>'
    tmux set -g pane-border-status top

Set `pane-border-status` only when it read `off`. If the format you read already starts with
`#{?@moai,`, it is shown already — leave it. If that value holds a single quote, do not build
the line; tell the person what to add. Never write the person's tmux config file.

## Clear a worker's window

Before you send a worker its next work, you may clear its window (`/clear`) yourself — all of
these first:

1. **Its report is checked** — the supervisor's 5 held all three checks and the
   `Report-checked:` note is written
2. **It reads `idle`** — in `ListAgents` and in its row of the session map
3. **Its input box is empty** (above)

Then:

    tmux send-keys -t <pane> /clear Enter

and, as a separate call, look once: the session map reads `idle` for it and `capture-pane`
shows the cleared screen — the conversation gone, an empty box. Then send with `SendMessage`
as the supervisor's 3 says. **If any condition fails, or the look does not show it cleared,
do not type again** — do what the supervisor's 5 says without tmux: ask the person, or send
to another idle worker. When it was the look that failed, tell the person that `/clear` may
stand typed in that pane's box: pressed later, it would erase the next message sent there.

## When a message does not arrive

When `SendMessage` to a worker fails — an error, no such session — and that worker has a pane
whose input box is empty (above), you may put the message into the box yourself. **A message
held for the person's approval is not one that failed:** that hold is the person's gate, like
a permission prompt, and the held message still arrives once they approve it — pasting it too
skips their gate and hands the worker the same work twice. Tell the person it waits for them
instead. Typed keys submit at every newline, so paste it as one block:

1. Write the message to a file in your scratchpad, with one line at the end naming you — a
   pasted message carries no sender, and the worker reports to the message's `from`. The
   first line stays the message's own, which names the work and the step to start from:
   `from: <your ListAgents name>`
2. Paste and submit it:

       tmux load-buffer -b moai-send <file>
       tmux paste-buffer -p -d -b moai-send -t <pane>
       tmux send-keys -t <pane> Enter

3. **Tell the person** you did, and into which pane

The worker still answers with `SendMessage`. A worker that answered your message by refusing
the work is not a delivery that failed — it comes out of the candidates (the supervisor's 2).

## When a worker stalls

When a `notify_when_idle` notice comes with no report, or the person asks about a worker that
has read `busy` far longer than its work should take, look at its pane once:

    tmux capture-pane -p -t <pane> -S -40

and read why it stopped — a permission prompt, a question (`AskUserQuestion`), an API or
rate-limit error, or a process that ended (a shell prompt where the box was). **Tell the
person** what the pane shows and which pane it is. Do not answer the prompt, do not press a
key, and do not look again in a loop. A process that ended is a dead session: bringing it
back is `moai-recover`, once the person asks for it.

## No idle worker left

When the supervisor's 2 finds no worker, ask the person **once** whether to open new worker
panes, and how many — one question covers several panes. Open a pane only on their yes, or
when they asked you for it. On a yes, for each pane:

    tmux split-window -P -F '#{pane_id}' -t "$TMUX_PANE" -c <root> 'claude --model <model>; exec bash'
    tmux select-layout -t "$TMUX_PANE" tiled

`<root>` is the `root dir` of the supervisor's 2 and `<model>` the model picked in 2-1. **No
other flag.** The first line prints the new pane's id; `exec bash` keeps a shell in the pane
when `claude` exits, so what it said stays readable. The pane belongs to the same person —
it is their interactive session like any other.

Once it has started (a separate call: `capture-pane` of the new pane shows the `❯` box), look
at `ListAgents` once more. The new row is a worker like any other (the supervisor's 2) — send
to it as in 3. If it does not show yet, look once more after your next step; if a prompt
stands in the new pane (trusting the folder, say), tell the person — it is theirs to answer.
