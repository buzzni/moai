## Workers in sessions

When the workers are sessions, **moai carries no messaging.** The supervisor and its
workers talk with Claude Code's own tools:

- `ListAgents` lists the live sessions, and subagents too — each row's name, its kind
  (`interactive`, `bg`), whether it is busy or idle, and its tmux pane if it has one
- `SendMessage(to: <name>, message: …)` sends to one session. With `notify_when_idle: true`
  you also get one notice when that session goes idle; leave `message` out and it only
  subscribes
- A reply comes in as a cross-session message. Answer it by copying its `from` as `to`

**Every session here is an interactive one the person can see.** Outside tmux, cmux and Saycode
the supervisor launches no session; inside one of them it may open a worker pane or
session, only after the person says yes (`moai-tmux`, `moai-cmux` or `moai-saycode`, below).
**A worker is every idle session of this
repository in `ListAgents`, except you** — a row whose name starts with the root
directory's slug and a `-` (2). Nobody registers and nobody is asked which windows count.
The message you send is the whole assignment, and it names the file of the worker's steps,
which the worker reads (3). A Codex worker's assignment also carries the repository's
optional `[supervise] extra_lines` just before `from:` — `moai supervise message` inserts them
(`moai-tmux`, `moai-cmux`).

Five things about the messaging, one line each:

- A session in a different permission mode holds an incoming message for its person's
  approval — a worker that stays idle after you sent may be waiting on that
- `notify_when_idle` answers only for a session on this machine — one reason a worker is a
  session on this machine (2)
- A subagent sends under its parent session's address — a message can come from a session
  that did not write it itself
- `@path` in a message attaches nothing — a file the worker has to read is named by its
  absolute path, and the worker reads it itself (3)
- Never poll `ListAgents` in a loop — the report comes to you (4)

**Inside tmux, load `moai-tmux` too.** When `$TMUX` is set in your shell — and is not cmux's
stand-in (the next paragraph) — the skill `moai-tmux` gives you hands on the workers' panes,
at five points of the round: offer new worker panes when no worker is idle (2), label the
pane you send to and deliver a message `SendMessage` could not (3), read a stalled worker's
pane (4), and clear a reported worker's window before its next work (5). It finds each
worker's pane from its `ListAgents` name, and never types into a box that holds anything.
**Without `$TMUX` nothing of it applies** — every step below goes through messages and the
person, as written, unless you are inside cmux (the next paragraph). `moai-tmux` is an optional
skill: it stands only where it was chosen (`moai skill install --with moai-tmux`, or its row in `moai
init`). Inside tmux without it, tell the person that one line plants it, and carry on as if
outside tmux.

**Inside cmux, load `moai-cmux` instead.** When `CMUX_SURFACE_ID` is set in your shell and
`$TMUX` is not, `moai-cmux` gives you the same hands on the workers' cmux tabs, at the same
five points, and reads the same session map. It needs cmux 0.65.0 or later and says so itself.
**A `$TMUX` that starts with `/tmp/cmux-` is cmux's own stand-in**, not a tmux server —
`cmux claude-teams` sets it for Claude Code's agent teams, and the `tmux` on its `PATH` is cmux
in disguise — so with it you are inside cmux: `moai-cmux`. When both are set otherwise you run in
tmux inside a cmux tab — what you type goes to tmux, so it is
`moai-tmux`. **With neither, nothing of either applies.** `moai-cmux` is optional the same
way (`moai skill install --with moai-cmux`); inside cmux without it, tell the person that one
line plants it, and carry on as if outside cmux.

**Inside Saycode, load `moai-saycode` too — it comes first.** When `SAYCODE_AGENT_ENV` is `1`
in your shell and Saycode's `whoami` answers ok, the skill `moai-saycode` drives every worker
Saycode lists at the same five points, through Saycode instead of a pane: it opens new worker
sessions (2), sends the work as a prompt and is told when the turn ends (3), reads a stalled
worker's state (4), and clears a reported worker (5). A Saycode session often runs inside tmux
or cmux as well; then `moai-tmux` or `moai-cmux` is only for a worker Saycode does not list,
and for the pane labels (the person's decision). **When several stand, Saycode comes first,
then tmux, then cmux** (tmux in a cmux tab is tmux, above). `moai-saycode`
is optional the same way (`moai skill install --with moai-saycode`); inside Saycode without
it, tell the person that one line plants it.


**A session.** Call `ListAgents` once. A worker is a row that

- belongs to this repository. `ListAgents` shows no directory; a session takes its name from
  the directory it was opened in, slugged — lowercased, every run of characters other than
  `a-z` and `0-9` turned into one `-`, cut at 4 words or 40 characters — then `-` and a short
  hex suffix: `moa-issue-bc` for `moa-issue`, `tvshop-updater-ca` for `tvshop_updater`. A row
  whose name does not start with the root's slug and a `-` — renamed, or opened somewhere
  else — is not one. **A name that does start so is still only a candidate**: `api-gateway-1c`
  starts with `api-`, and a session opened in a clone named `moai-web` starts with `moai-`.
  The worker confirms it: the message carries `Root:`, and a session standing in
  another repository refuses the work, so it comes out of the candidates (below)
- is a session a person opened — under "Peer sessions" and `interactive`. Not a subagent,
  yours or another session's (they stand under "Subagents", and a message to one resumes that
  subagent instead), and not a `bg` session
- runs on this machine — a Remote Control or cloud session cannot read the steps file at the
  path you name (3), and sends no idle notice
- reads `idle`
- is not you, and not a supervisor (one per repository, above)
- inside Saycode, is not a session Saycode started — those are counted from Saycode's own list,
  so none is counted twice (`moai-saycode`, "Which session is which worker")

Its name is what you send to. Nobody registers: any idle session a person opened here is a
worker, and the message is the whole assignment.

- **Leave out a worker whose sent work has not had its report checked.** It goes idle
  whenever its turn ends — while it asks its person something, say — and it is still
  holding your work. A resumed window comes back under a new name (0): while a worker you
  sent to is gone from `ListAgents` with its report unchecked, a name you have not sent to
  may be that worker — ask the person before sending to it
- **If no row is left, nobody is free here.** Tell the person, and stop — do not send to a
  session of another repository. Inside Saycode, ask the person first whether to open new
  worker sessions (`moai-saycode`, "No idle worker left"). Otherwise, inside
  tmux, ask the person first whether to open new worker panes
  (`moai-tmux`, "No idle worker left"); inside cmux, whether to open new worker tabs
  (`moai-cmux`, "No idle worker left"). On a no, stop
- **A worker that refused the work comes out of the candidates and is not sent to
  again.** Some sessions take work only from their own person
- **A test agent is no worker.** One raised for a test is opened outside the repository (a
  scratchpad), so its name is not this repository's


**A session's report comes back as a cross-session message** from the worker,
`report: <epic>` at its head, and it wakes you. The idle notice that
`notify_when_idle` sends is not a report: a worker goes idle whenever its turn ends — while
it waits on its person's answer, say — and one that asked its person something sends nothing
until it is answered. **Do not poll `ListAgents`** — the report comes to you. Inside tmux, an
idle notice with no report is the moment to read that worker's pane once (`moai-tmux`, "When a
worker stalls") and tell the person what it shows — inside cmux, its tab (`moai-cmux`, "When a
worker stalls"). Inside Saycode, that moment is the wait ending with no report —
`moai-saycode`, "Take the report" and "When a worker stalls".


For a session worker, if the supervisor is in the root, then in the gap after the worker
picks the member up and before it raises its worktree, the hook may hold that member as "still picked up" when
the supervisor's turn ends. **That member is the worker's** — do not move it, do not
defer it, do not put a note on it; just finish the turn.

