# Working with AI agents

moai is built so that AI agents run the tracker beside a person. There is no
approval gate: an agent creates, moves and closes work without asking, and the
one thing it asks about is picking up work that belongs to someone else — a
[take over](glossary.md#take-over). This page covers the three things that make
that work — the instructions an agent reads, the skills and hooks planted into
Claude, and the `--json` surface a loop of your own can drive. The commands'
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

### Where the guide goes

The first `moai init` in a terminal asks, and `--guide` says it without asking:

| `--guide` | What `init` writes | When to pick it |
|---|---|---|
| `block` | the whole guide inside the `AGENTS.md` block | what `init` has always done, and still does when a script or an agent runs it |
| `file` | the guide in `.moai/guide.md`, and a few lines in the block that point at it | `AGENTS.md` is yours and you want it short — the screen's pick for a committed tracker |
| `hook` | nothing in `AGENTS.md`; the hooks tell Claude instead | the tracker is kept out of git — the screen's pick there |
| `none` | nothing (`--no-agents`) | you hand the guide over some other way |

- **`file` keeps one way to the text everywhere.** The link names
  `moai init --print` as well, so a clone that lacks the file still gets the same
  guide from the binary. `--check` and `moai status` measure the link and the file
  both, and a `.moai/guide.md` edited by hand reads as stale — `init` writes it again
- **`hook` needs the hooks installed**, so `init` installs them with it
  (`moai skill install --scope local`) and refuses `--no-skill` beside it. The
  first prompt of a session then carries the board and one line saying the usage
  is in the `moai` skill. That line appears in any checkout whose `AGENTS.md` has
  no moai block, and only there
- **Nothing records which one you picked.** Run again, `init` reads it from the
  block (a link means `file`); a tracker kept out of git is left without a block

## Plant the skills and hooks into Claude

    moai skill install                     just me (the default)
    moai skill install --scope project     the whole team, through the committed settings
    moai skill status                      what is installed where, and what differs

`skill install` writes the plugin into `.claude/moai-plugin/` and registers it
with `claude`; your `settings.json` is `claude`'s to write. It is safe to run
again, and a Claude session that is already open keeps the old copy until you
reopen it. `moai skill uninstall` takes the registration away and leaves the
files. The skills and hooks are for Claude Code today; other agents get the
`AGENTS.md` block and the `--json` surface.

Three skills come with it:

    moai              the tracker itself — what to pick up, issues, plans, ideas
    moai-supervise    hands piled-up ideas to the sessions idling on the repository
    moai-wiki         keeps this wiki in step with the work

- **`moai`** is the tracker skill — what an agent reaches for instead of a
  to-do list of its own
- **`moai-supervise`** makes a session the [supervisor](glossary.md#supervisor),
  which hands the [ideas](glossary.md#idea), one at a time, to the Claude
  sessions idling on the same repository and takes their reports. It picks, sends and checks; it does
  not fix and it does not merge. The [workers](glossary.md#worker) follow a
  numbered [brief](glossary.md#brief), and their way of working is on
  [the workflow page](workflow.md#work-in-a-worktree)
- **`moai-wiki`** is followed by the window that did an epic, to fix the page
  that teaches what the epic changed; a person can also call it to sweep
  everything merged since the last release

## What the hooks do

The plugin hooks four places in a Claude session. Each calls `moai hook`, which
nobody runs by hand.

| When | What happens |
|---|---|
| The session starts | A baseline of the warnings is written. After a compaction, what the session was holding is loaded back into it |
| A person sends a prompt | The `moai status` board is loaded, once per session |
| Before a tool call | The five rules below are checked |
| The turn ends | If the session still holds work, the turn is held once and asks for a [`Next:` note](glossary.md#next-note) for whoever comes after; it is also held when the [warnings](glossary.md#warning) grew |

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
   the plan. Not counted: `.moai/`, `.claude/`, `.git/`, `target/`,
   `node_modules/`, and anything outside the repository
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

## Run agents without a person

Every command takes `--json`. Three are enough for a loop:

- `moai ready --json` — `ready` is yours to pick up, `others` is ready work that
  belongs to someone else or nobody (ask first), `held` is what is deferred or
  blocked and where to pick it up again
- `moai mv <id> in_progress --from todo` — picks up one id only if it still
  stands where you saw it. A session that lost the race gets `stale` and a
  non-zero exit, and moves on to the next row. Pass one id per call
- `moai prime` — what this session holds and what is next, short enough to load
  into a prompt

`examples/bash-agent/agent.sh` is a whole pick-work-close loop in bash and jq, and
`examples/python-agents/agents.py` runs several agents at once. The test suite
runs both.

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

Decided in: moai-2w0s moai-hxma moai-0zjo moai-nqdc moai-bl3x moai-gelm moai-tllo moai-mdzx
