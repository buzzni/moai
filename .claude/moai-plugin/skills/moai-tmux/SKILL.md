---
name: moai-tmux
description: Use in Claude Code together with moai-supervise when the supervisor runs inside tmux ($TMUX is set and is not the /tmp/cmux- stand-in that cmux claude-teams sets) — to find a worker's pane from its ListAgents name, label it, clear a reported worker's window, deliver a message SendMessage could not, read why a worker stalled, and open new worker panes once the person says yes. Triggers on "worker pane", "open a worker", "clear the worker", "일꾼 칸", "일꾼 창 열어", "칸 비워".
---

# moai-tmux — the supervisor's hands on the workers' panes

This is the supervisor's (`moai-supervise`) companion **when it runs inside tmux** — `$TMUX`
is set in its shell. Without `$TMUX` none of this applies: the supervisor works through
messages and the person — or, inside cmux, through `moai-cmux`. Nor does it apply when `$TMUX`
starts with `/tmp/cmux-`: that is cmux's own stand-in (`cmux claude-teams` sets it), and its
`tmux` is cmux in disguise — that is `moai-cmux`'s. The tmux server here is **the
person's own**: every session they have lives on it, and every pane you touch is one they are
looking at.

## What never happens

- **No headless run.** A worker you open is an ordinary interactive `claude` the person can
  see and type into. Never `claude -p`, never a `--dangerously-*` flag, never
  `--permission-mode`. The moai binary launches and drives nothing; this skill is you
  typing tmux commands where the person can watch
- **Nothing is killed.** Never `kill-server`, `kill-session` or `kill-pane`, never `pkill`
  or `killall` aimed at tmux — one bare `kill-server` once took down every session the
  person had (hook rule 4, and the person's own guard hook refuses it too). A worker that
  is done stays open; closing a pane is the person's
- **Only a worker's pane.** Type only into a pane found through the session map below (or
  the Codex process checks in "Codex workers") for a
  worker of this repository — never your own pane (`$TMUX_PANE`), never a session of another
  repository, never a test server's pane. Every call names its pane, `-t <pane>`, as `%N`
- **Never over the person's words.** If a worker's input box holds anything, do not type into
  it — tell the person
- **Never in their place.** A permission prompt or a question in a worker's pane is the
  person's to answer — you read it and tell them; you press no key
- **No pane polling.** Every look at a pane below is one look. A second one comes after your
  next step, as its own call — never a loop and never a `sleep` for screen reads. The Codex
  report monitor reads only this repository's tracker, as "Codex workers" says
- **The person's tmux config file stays theirs.** What you set is on the running server

## Which pane is which worker

`ListAgents` names a session; tmux needs a pane. Claude Code keeps one record per process
under `~/.claude/sessions/`, and the lines below print one row per record, tab-separated:

    state  name  pane  status  cwd  sessionId  saycode

Read `../moai-supervise/references/session-map.md` and run its session map.

- **Read only `alive` rows.** A `dead` row is a record a process left behind — its pid is gone,
  or now belongs to another process (the start time differs)
- `name` is the `ListAgents` name, `pane` the `%N` that `-t` takes — `-` when that session
  is not in tmux or runs on another tmux server than yours (pane ids are counted per server,
  so another server's `%4` is a different pane here), and then this skill has nothing for
  that worker
- **No row at all, and a line saying no record reads alive:** this shell cannot tell live from
  dead — act on no row, and tell the person what it said
- **A session Saycode's daemon started has no pane** — it prints `-`. Its record carries the
  pane the daemon was started from, and typing there would type into that pane, not the
  worker's. **A pane two alive rows name belongs to neither** either — both print `-`, so
  leave both alone
- **A row whose `saycode` is not `-` is a Saycode session.** Inside Saycode (`moai-saycode`) it
  is driven through Saycode, not its pane — you type nothing into it, and only label it
- `cwd` is where the session stands: the root, or one of the worktrees `git worktree list`
  names — wherever they stand. A row standing elsewhere is not a worker of this repository, whatever its name
- `status` is `idle`, `busy` or another word. It has to agree with `ListAgents` where a step
  below asks for `idle`
- Your own row is the one whose pane is `$TMUX_PANE`; a row of yours that prints `-` has no
  pane this skill can use. Inside a session Saycode's daemon started, `$TMUX_PANE` is the
  daemon's pane, handed down — not yours

Run it when a step below needs a pane, and once more just before you type into one — a reading
from earlier in the round may be a worker that has died since (the next section).

## Is the input box empty

    tmux display -p -t <pane> '#{pane_in_mode} #{pane_dead}'
    tmux capture-pane -p -e -t <pane>

The first line prints two values. The first is `1` while the person is scrolling the pane (copy
mode) — keys you send then go to tmux's copy mode, not to Claude Code, so a pane in a mode is
theirs: do not type. The second is `1` once the pane's program has exited — a pane a moai skill
opened keeps its last screen under `remain-on-exit` and takes no keys: the session there is
dead, so do not type. When it prints neither value, the pane is gone: do not type either.
Claude Code's input box is the line that starts with `❯`, under the conversation, between two
`─` rules. **Empty** is `❯` followed by nothing, or by Claude Code's dim placeholder — `-e`
keeps the colours, and the placeholder is drawn dim (SGR `2`, or a grey foreground) where the
person's text is not. Anything else — a word, a pasted block, a half-typed command — is the
person's, and if you cannot tell the placeholder from their draft, it is theirs. A pane with
no `❯` box at all (a shell prompt, a dialog) is not a box to type into either. **Even an empty
box counts only while the session map, run again just before, still reads that session
`alive` in that pane** — the screen alone cannot tell: in a pane the person opened, a Claude
Code that was killed keeps its last frame, box and all, above the shell it ran in, the second
value stays `0`, and the keys go to that shell.

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
rate-limit error, or a process that ended (a shell prompt where the box was, or under its last
box — a killed Claude Code leaves its last screen behind — or, in a pane you opened, a last line
`Pane is dead (…)` that `remain-on-exit` left under it); whether the session is still alive,
the session map says. **Tell the person** what the pane shows and which pane it is. Do not answer the prompt, do not press a
key, and do not look again in a loop. A process that ended is a dead session: bringing it
back is `moai-recover`, once the person asks for it.

## No idle worker left

When the supervisor's 2 finds no worker, ask the person **once** whether to open new worker
panes, and how many — one question covers several panes. Open a pane only on their yes, or
when they asked you for it. On a yes, for each pane:

    tmux split-window -P -F '#{pane_id}' -t "$TMUX_PANE" -c "<root>" 'claude --model <model>' \; set-option -p remain-on-exit on
    tmux select-layout -t "$TMUX_PANE" tiled

`<root>` is the `root dir` of the supervisor's 2 and `<model>` the model picked in 2-1 — keep
the double quotes, so a path with a space stays one argument. **No other flag.**

The first line prints the new pane's id (`%N`), the `<pane>` below. The `set-option` after
`\;` runs in the same tmux call, on the new pane, before the program can exit — set in a call
of its own, a program that ended at once would have taken its pane, and what it said, with it.
`remain-on-exit` keeps the pane when the program exits: its last screen stays readable,
`#{pane_dead}` reads `1`, and tmux refuses pastes aimed at it and drops keys — nothing you send
lands in a shell under a dead session, though `send-keys` still exits `0`. A dead pane stays
until the person closes it; never kill it.

The pane belongs to the same person — it is their interactive session like any other.

Once it has started (a separate call: `capture-pane` of the new pane shows the `❯` box), look
at `ListAgents` once more. The new row is a worker like any other (the supervisor's 2) — send
to it as in 3. If it does not show yet, look once more after your next step; if a prompt
stands in the new pane (trusting the folder, say), tell the person — it is theirs to answer.

## Codex workers

These are interactive Codex sessions, each in a pane, selected as `Worker: codex` — not
Claude Code subagents. The supervisor remains Claude Code only. The Claude session map
above is not a Codex registry: do not add Codex to `SESSIONS` or expect `ListAgents`,
`SendMessage` or `notify_when_idle` to reach one. Use the epic notes for its reports.

Read `../moai-supervise/references/codex-workers.md` for the shared Codex model and review rules.

### Find the Codex pane

On the running server (`$TMUX`), read the panes once:

    tmux list-panes -a -F '#{pane_id}<TAB>#{pane_current_command}<TAB>#{pane_current_path}<TAB>#{pane_pid}'

Replace each `<TAB>` with a literal tab. A candidate's `pane_current_command` is `codex`,
and its `pane_current_path` is `<root>` or a worktree named by `git worktree list`.
Resolve path aliases before comparing (on macOS `/tmp` can be `/private/tmp`). A launcher
can leave `pane_current_command` as `node` or a shell such as `zsh`; inspect the pane PID
and two levels of its descendants with
`ps -axo pid=,ppid=,args=` — under the pane's shell an npm install runs `node …/bin/codex`,
which starts the native `codex` — and confirm one by the live PID rule below, not a process
whose arguments merely mention Codex. If you cannot identify a live Codex
process in that pane, do not type. Repeat this check just before each input — an exited
Codex can leave its last frame over a shell. Never use another server's pane ids, your own
`$TMUX_PANE`, or a pane outside this repository.

### Read the Codex input box

Run `python3 "<moai-tmux skill dir>/scripts/codex-pane.py" ready <pane>` immediately
before input, from `<root>`. It checks mode, liveness and the composer and refuses uncertainty,
and it refuses a pane whose current path lies outside the checkouts `git worktree list` names
there. Identify the pane as this repository's first all the same. The lowest current composer starts with `›` (U+203A) and an ordinary space (U+0020).
Its empty placeholder is `Ask Codex to do anything`. A past user message also starts with
`›`: read the bottom input area, not any matching line in the conversation.

The placeholder alone is not an idle box. During a response it can remain visible beside
`Working` and a spinner; a past `Worked for …` is different. Do not type while a current
response, draft (including continued lines), approval prompt, unanswered question, slash
menu or work directory menu is visible. A cropped or contradictory screen is the person's:
do not type; tell them which tab or pane and what could not be read.

Codex is absent from the Claude session map. Just before typing, re-read the process list
and check a live Codex PID: use cmux top's `cmux_process_pids`, or tmux's `pane_pid` and
its process tree, with `ps -p <pid> -o args=`. Check the first executable's basename is
`codex`; a truncated `comm` or a child such as `codex-code-mode-host` is not that proof, nor
is an npm install's `node …/bin/codex` launcher — the native `codex` it starts is.
Require that PID, a clear empty composer and the completed report checked by the supervisor
(or a fresh startup screen before its first assignment) together. A shell under a dead
Codex's old composer is not a box to send work into.

cmux's `surface.input_state` can partly recognize Codex drafts and dialogs, and guards can
refuse a paste. They can also leave stale lifecycle values, miss questions or menus, or read
`empty` while Codex is working. Do not use that state as the Codex idle test and do not rely
on a paste refusal to protect a draft: a fresh unregistered agent can accept text over it.
If any check is unclear or any input is refused, do not type again; ask the person.

### Open a Codex worker

When no idle Codex worker is left, ask the person **once** whether to open new panes and
how many. Open one only on their yes, or when they asked for it. On a yes:

    tmux split-window -P -F '#{pane_id}' -t "$TMUX_PANE" -c "<root>" 'codex --model <codex>' \; set-option -p remain-on-exit on
    tmux select-layout -t "$TMUX_PANE" tiled

The first line prints the new pane's id (`%N`), the `<pane>` below. The `set-option` after
`\;` runs in the same tmux call, on the new pane, before the program can exit — set in a call
of its own, a program that ended at once would have taken its pane, and what it said, with it.
`remain-on-exit` keeps the pane when the program exits: its last screen stays readable,
`#{pane_dead}` reads `1`, and tmux refuses pastes aimed at it and drops keys — nothing you send
lands in a shell under a dead session, though `send-keys` still exits `0`. A dead pane stays
until the person closes it; never kill it.

This is an ordinary interactive session. Read the new pane separately and confirm
the live process and empty composer above. Folder trust and hook trust prompts belong to
the person; tell them what the pane shows. Do not assume cmux's wrapper enabled or trusted
hooks here. The person checks moai in `/hooks`; the worker keeps the five rules even when
no hook stands.

### Clear a reported Codex worker

Only after `Report-checked:` and its work has stopped, run
`python3 "<moai-tmux skill dir>/scripts/codex-pane.py" clear <pane>`. The helper checks
before typing, verifies `/clear` before Enter and checks the fresh composer. If it refuses,
stop and tell the person; a menu, question or permission prompt is theirs.

### Paste the assignment

The helper is `scripts/codex-pane.py` in the moai-tmux skill. Assemble the assignment with
`moai -C "<root>" supervise message <id>`, giving `--model` (`<codex>` and the difficulty), `--effort`,
`--base`, `--root-branch` and `--from` (your ListAgents name) plus the other slots its `--help` lists —
an epic goes from step 2, a backlog item from step 1. Its first line names the absolute path to `references/worker-codex.md`
beside `worker.md` in `moai-supervise`; pass `--steps <that path>` when the skill is not planted under `<root>`. Lines the
repository wrote under `[supervise] extra_lines` in `.moai/config.toml` stand just before `from:`. Save it
under the epic scratch directory. Confirm the fields, those lines included, and the person's approval, then run
`python3 "<moai-tmux skill dir>/scripts/codex-pane.py" send <pane> <file>`; it rechecks
before pasting and before submission. Tell the person which pane received it. "Label the
pane" applies unchanged: set `@moai` on this pane and ask before showing labels.

### Watch the epic for the Codex report

Before sending, record the latest `journal[].ts` of `moai -C "<root>" show <epic> --json`
(an empty journal uses `1970-01-01T00:00:00Z`). Arm a `Monitor` with
`moai -C "<root>" watch <epic> --since <timestamp>`; it prints one JSON event containing
`head`, `ts`, and a resumable `cursor`. It returns immediately for a matching existing
note — `--since` is inclusive, so an event at the recorded timestamp may be a note you
already handled: check it, and if so re-arm with its `cursor`. To continue, start the next monitor with `--cursor <cursor>`. Keep the cursor on
expiry; identical notes in the same second are one event. A failed read is an alert.
Watch the backlog until `Unfolded:`, then watch its new epic from the beginning. On
`Decision needed:` ask the person, on `Review-ready:` start review, and on `report:`
check the final report and write `Report-checked:`. Never poll the worker screen.
