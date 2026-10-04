# Working with AI agents

moai is built so that AI agents run the tracker beside a person. There is no
approval gate: an agent creates, moves and closes work without asking, and the
one thing it asks about is picking up work that belongs to someone else — a
[take over](glossary.md#take-over). This page covers what makes that work — the
instructions an agent reads, the skills planted for Claude Code, Codex and
Antigravity (with hooks for Claude Code), the letters agents leave each other,
and the `--json` surface a session reads its queue from. The commands'
flags are in [the CLI reference](cli.md), and the words in
[the glossary](glossary.md).

## Tell the agent how the tracker works

`moai init` writes a managed block into `AGENTS.md`: every command an agent needs
and the three forks it meets (create or idea, defer or done, split into an epic or
not). The block sits between `<!-- moai:begin … -->` and `<!-- moai:end -->`, and
only the block is rewritten — your own prose around it stays, and the issues and
the journal are never touched.

- **Claude reads `CLAUDE.md`, not `AGENTS.md`.** `init` does not copy the block
  there (two copies would drift); put one `@AGENTS.md` line into `CLAUDE.md` and
  both are read. `init` says so when it sees a `CLAUDE.md` without it
- **An agent that reads some other file** takes the same block from
  `moai init --print`, which writes nothing
- **When the tool grows, run `moai init` again.** `moai status` says when the
  block is stale — written by another binary (build again, then `init`) or edited
  by hand (`init` throws that edit away). `moai init --check` answers `current`,
  `stale` or `missing` and writes nothing

## Plant the skills

    moai skill install                     Claude Code, just me (the default)
    moai skill install --scope project     Claude Code, the whole team through the committed settings
    moai skill install --agent codex       Codex and Antigravity, through the committed .agents/skills/
    moai skill install --agent auto        whichever of claude, codex and agy is on PATH
    moai skill status                      what is planted where, and what differs

`--agent` names who the skills are for — `claude` (what you get when it is left
out), `codex`, `antigravity` or `auto` — and can be repeated.

- **Claude Code gets a plugin.** `skill install` writes it into
  `.claude/moai-plugin/` and registers it with `claude`; your `settings.json` is
  `claude`'s to write, and `--scope` picks where it registers. A Claude session
  that is already open keeps the old copy until you reopen it. The hooks come
  with the plugin, and only with it
- **Codex and Antigravity read the same `.agents/skills/`** in the repository, so
  naming either writes it for both. There is nothing to register — commit the
  directory and the team has the skills. `--scope` does not apply to them, and
  one line says so when you give it without Claude
- **`auto` looks at PATH** for `claude`, `codex` and `agy`, says what it found,
  and plants for Claude when it finds none of them

It is safe to run again: files are only overwritten, never deleted. `moai skill
uninstall` takes Claude's registration away and leaves the files; with
`--agent codex` it prints the `rm -r` lines for moai's skills in
`.agents/skills/` and deletes nothing. `moai skill status` shows Claude's
registration, whether `.agents/skills/` holds this version's skills, and whether
`codex` and `agy` are on PATH — and exits 0 whatever it finds.

**One text serves every agent.** Both trees get the same skills. The steps only
one agent has — entering a worktree, asking the person, calling the review,
changing the model, clearing the window, calling a skill, waking a session,
stopping what a review left running — sit in a "Words per agent" table in the
`moai`, `moai-supervise` and `moai-work` skills, one column per agent, and each
agent reads its own; the steps name a row in *italics*. A step an agent does not
have reads `—`: tell the person and go on.

Four skills come with it:

    moai              the tracker itself — what to pick up, issues, plans, ideas
    moai-supervise    hands piled-up ideas to the workers waiting on the repository
    moai-wiki         keeps this wiki in step with the work
    moai-work         makes a window a worker that waits for those ideas

- **`moai`** is the tracker skill — what an agent reaches for instead of a
  to-do list of its own
- **`moai-supervise`** makes a session the [supervisor](glossary.md#supervisor),
  which hands the [ideas](glossary.md#idea), one at a time, to the
  [workers](glossary.md#worker) waiting on the same repository and takes their
  reports. It picks, sends and checks; it does not fix and it does not merge
- **`moai-work`** makes a window a worker: it waits for a supervisor's letter and
  does the work by the numbered [brief](glossary.md#brief) the skill carries. Both
  are in [Hand work to waiting workers](#hand-work-to-waiting-workers), and the
  worker's way of working is on [the workflow page](workflow.md#work-in-a-worktree)
- **`moai-wiki`** is followed by the window that did an epic, to fix the page
  that teaches what the epic changed; a person can also call it to sweep
  everything merged since the last release

## What the hooks do

The plugin hooks four places in a Claude session. Each calls `moai hook`, which
nobody runs by hand.

| When | What happens |
|---|---|
| The session starts | A baseline of the warnings is written and the session's [presence](glossary.md#presence) row is written, idle. After a compaction, what the session was holding is loaded back into it, with any [letters](glossary.md#letter) for it, and busy or idle stays as it was |
| A person sends a prompt | The `moai status` board is loaded, once per session. The letters for the session are loaded every time, and it is marked busy |
| Before a tool call | The five rules below are checked — nothing else; the mailbox is not opened |
| The turn ends | Letters for the session hold the turn first — not the ones it sent itself, which the next prompt loads. Then, if the session still holds work, the turn is held once and asks for a [`Next:` note](glossary.md#next-note) for whoever comes after; it is also held when the [warnings](glossary.md#warning) grew. A turn that ends is marked idle |

**The hook never fails the session.** Whatever goes wrong inside it, it exits 0,
and the only thing it refuses is the one tool call that broke a rule. A person
typing `moai` in a terminal never passes through it.

## The five rules

A refusal opens with the rule's number and name, and hands over the command that
gets through — run it as given. None of them waits on a person except rule 5.

1. **New issues stay inside what you picked up.** While an agent holds work (its
   [focus](glossary.md#focus)), a `moai add` must land in the same
   [epic](glossary.md#epic) (`-e <epic>`) or under the held issue
   (`--parent <id>`). Something for later goes in as `moai idea add`, which this
   rule never stops; nor does it stop a whole plan created with `moai add --from`
2. **Pick something up before you change the repository.** An `Edit`, a `Write`,
   or a shell write (`>`, `>>`, `sed -i`, `tee`) to a file in the checkout needs a
   held issue — `moai mv <id> in_progress`, or `moai add` first if it was not in
   the plan. Not counted: `.moai/`, `.claude/`, `.worktrees/`, `.git/`,
   `target/`, `node_modules/`, and anything outside the repository
3. **A review is an issue too.** `/code-review` needs an open review issue tied to
   the held work, and that issue needs an angle in its body (`-b`) — what is being
   looked for and why. Moving it to `done` needs a closing line (`-m`) saying what
   was taken in and what was handed on
4. **Never kill the person's tmux server.** `tmux kill-server` or `kill-session`
   without `-L`/`-S`, and `pkill`/`killall` aimed at tmux, are refused even
   outside a tracker. A tmux under test gets its own server:
   `env -u TMUX tmux -L <unique name> …`
5. **Ask before you pick up someone else's work.** Moving a row assigned to
   someone else, or to nobody, into a started column is refused without `--take`.
   The agent asks the person watching, and on a yes runs
   `moai mv <id> in_progress --from todo --take -m '<who said yes>'` — it becomes
   the assignee and a `Taken-over:` note keeps whose it was (moai-0zjo)

**Who the agent is** comes from `--user "Name (email)"`, then `MOAI_ACTOR`, then
`git config`. When none of them says, a write stops and asks for one — reads
never ask — and rule 5 sets nothing apart.

## Leave each other letters

Agents talk through a mailbox under `.moai/` — a [letter](glossary.md#letter) is
one file, and who is around is one [presence](glossary.md#presence) file per
agent.

    moai hello --role worker                 register this agent (the hooks do it for Claude)
    moai agents                              who is here, busy or idle; sweeps the gone
    moai agents --role worker --status idle  the workers waiting for work
    moai send <agent> '<subject>' -b -       leave a letter, the body from stdin
    moai send any-idle-worker '<subject>'    one agent takes it, not you or a supervisor
    moai inbox --ack                         the letters for you, marked read
    moai inbox --ack --wait 600              wait up to ten minutes for one

- **A letter is delivery, not record.** Nothing goes into `issues.jsonl` or the
  journal; a decision still goes on its issue as a note, and a report names the
  issue it is about. The mailbox follows the tracker into the main checkout, so
  every session of a repository — in any worktree — sees one mailbox, and
  `moai init` keeps both directories out of git
- **Names are what you send to.** A Claude session is registered by its hooks
  under the name Claude Code shows for it (the one `SendMessage` uses);
  `moai hello --name` picks another. A name is letters, digits, `.`, `_` and `-`.
  `moai send` and `moai inbox` know who you are from `--as`, `MOAI_AGENT`, or the
  registered agent they run under
- **`any-idle-worker`** is a recipient, not a name: the first agent that is
  neither the sender nor registered as a `supervisor` (`moai hello --role
  supervisor`) to take the letter keeps it. A hook takes one such letter per
  load, and so does one `moai inbox --ack`, so several of them spread over
  several agents; an `--ack` that tried in the same moment is told it was taken
- **The hooks deliver.** With the plugin installed an agent rarely runs
  `moai inbox`: each prompt and each turn's end load the letters for that session
  and mark them read — `moai inbox --all` shows them again, nothing is deleted.
  One load stays inside the 10,000 characters Claude Code carries per hook —
  the board included, on the first prompt — and says how many still wait; a
  letter too long for that is cut there, naming `moai inbox --all` for the
  rest. A letter's body holds up to 64 KB, its subject 200 characters
- **A worker waits for its letters.** `moai inbox --ack --wait` at the end of
  each task is how a worker session — one a person opened — gets its next one;
  nothing has to wake it. While it waits its row reads idle, and once a letter
  comes, busy — a wait that runs out leaves it idle. That is what
  `moai agents --status idle` finds: a worker waits inside its turn, where no
  hook marks it
- **Waking is a bonus.** `moai send --wake` knocks once on an idle recipient:
  when its presence row carries a tmux pane, `moai inbox` is typed into that
  pane; a Claude session cannot be woken from a command line, so the sender
  wakes it with `SendMessage`, as the printed line says. With neither, nothing
  happens and nothing is said — not everyone runs tmux. moai never runs an
  agent's own program to wake it. An agent at work is left alone

Every one of these takes `--json`. `moai inbox --json` gives `me`, the `letters`
(each with `id` and `read` besides the letter's own keys) and `lost` — the ids
another agent took first.

## Hand work to waiting workers

The supervisor and its workers are sessions a person opened — Claude Code, Codex
or Antigravity in any mix, any of them in either role. moai never launches one or
runs one headless, and nobody needs tmux.

1. **Make the workers.** In each window that should take work, call the
   `moai-work` skill once (`/moai-work` in Claude Code, `$moai-work` in Codex,
   by name in Antigravity). The window says `moai hello --role worker` and waits
   with `moai inbox --ack --wait`. Each wait that runs out costs one short turn of
   tokens before it waits again
2. **Make the supervisor.** In one window, call `moai-supervise`. It says
   `moai hello --role supervisor`, picks ideas that do not collide with the work
   open, finds the waiting workers with `moai agents --role worker --status idle`
   and sends each one idea as a letter
3. **The letter carries the assignment only** — the idea, the model and
   difficulty picked for it, the work running alongside, the base branch, the
   milestone, the root, whether to wait again or end the turn after the report,
   and whether the person is away. The steps are in the worker's skill
4. **The worker does the work in a worktree** — unfolds the idea into an epic,
   picks the members up, works in `<root>/.worktrees/<epic>`, has the epic
   reviewed inside its own session, merges, closes, leaves a `Next:` note and,
   last of all, reports with `moai send`
5. **The supervisor checks the report** — the merge is on the base branch, the
   epic is done, the worktree is gone — and sends the next idea

**The review runs inside the worker's own session** — `/code-review` in Claude
Code, the review the session has in Codex and Antigravity, or the worker reading
the diff itself. It never starts another agent (`codex review`, `codex exec`,
`agy -p`).

**Clearing a window is the person's, or the supervisor's on tmux.** A worker
loaded with one epic's conversation may be cleared between tasks: stop the wait,
clear it (`/clear`, `/new` in Codex) and call `moai-work` again — the context
lives in the tracker. A supervisor running inside tmux — any vendor — clears a
Claude Code worker's window itself once the report checks out: it reads the input
box first so a person's half-typed text is copied out, never typed over, and the
letter told that worker to end its turn instead of waiting. Once the clear went
through it sends the next letter and types `moai inbox` into the emptied box, and
the hooks load the letter. Codex and Antigravity windows are left to the person —
their input box is not read yet.

**When the person steps away**, they tell the supervisor, and the letters say
`Person: away`. The worker then settles a design question by its own
recommendation and leaves `Decided alone: …` on the issue instead of waiting,
and stops at anything that cannot be undone.

## Work the queue from a session

Every command takes `--json`. Three are enough for a session to take its next row:

- `moai ready --json` — `ready` is yours to pick up, `others` is ready work that
  belongs to someone else or nobody (ask first), `held` is what is deferred or
  blocked and where to pick it up again
- `moai mv <id> in_progress --from todo` — picks up one id only if it still
  stands where you saw it. A session that lost the race gets `stale` and a
  non-zero exit, and moves on to the next row. Pass one id per call
- `moai prime` — what this session holds and what is next, short enough to load
  into a prompt

moai never launches a session or runs one headless. A person opens each one, and
the session reads these the way that person would.

**Name the AI that did the work** before closing an issue — one note, the
[model line](glossary.md#model-line), one line per issue, read back as `work` in
`--json` and summed by `moai stats`:

    moai note <id> 'model: anthropic/opus-5 tokens=182000 (high — the write path)'

Leave `tokens=` out when the count is unknown; never write 0 or a guess.

## When it goes wrong

- **A tool call was refused.** Read the first line — `Rule N — …` — and run the
  command the refusal hands over. Do not stop to ask a person unless it is rule 5
- **The rules do not seem to stand.** `moai skill status` says whether the plugin
  is registered and whether the binary the hook calls still runs. A session opened
  before the install keeps the old hook until it is reopened
- **`skill install` printed two `claude plugin` lines and failed.** The
  registration did not go through — most often `claude` is not on `PATH`; run
  those lines where it is
- **The board says the `AGENTS.md` block differs.** Another binary wrote it, or a
  person edited it. Build the moai you mean to run and call `moai init`
- **The agent keeps writing into a worktree's `.moai/`.** It does not need to: a
  `moai` run inside a linked worktree writes the main checkout's tracker by itself.
  See [the workflow page](workflow.md)
- **The supervisor finds no worker.** `moai agents --role worker --status idle`
  lists only windows where a person called `moai-work` and whose row reads idle.
  A session the hooks registered has no role until it says hello as a worker; a
  worker busy on its person's work reads busy
- **A worker never answers.** A letter waits in `.moai/mail/` until that worker
  waits again or its hooks load it — `moai inbox --all --as <worker>` shows what
  it has. A row reads idle while its window waits, but also once a turn ended
  without waiting again (its person stopped the wait, or cleared the window); a
  letter sent then sits until that window's next prompt. Without hooks or tmux
  nothing wakes a window that is not waiting

Decided in: moai-2w0s moai-hxma moai-0zjo moai-nqdc moai-bl3x moai-gelm moai-tllo moai-mdzx moai-xs2h moai-h8tn moai-snyk
