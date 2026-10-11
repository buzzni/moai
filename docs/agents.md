# Working with AI agents

moai is built so that AI agents run the tracker beside a person. There is no
approval gate: an agent creates, moves and closes work without asking, and the
one thing it asks about is picking up work that belongs to someone else — a
[take over](glossary.md#take-over). This page covers what makes that work — the
instructions an agent reads, the skills and hooks planted for Claude Code, Codex
and Antigravity, the supervisor that hands work to idle Claude Code sessions,
and the `--json` surface a session reads its queue from. The commands'
flags are in [the CLI reference](cli.md), and the words in
[the glossary](glossary.md).

## Tell the agent how the tracker works

`moai init` writes a managed block into `AGENTS.md`: every command an agent needs
and the three forks it meets (create or backlog, defer or done, split into an epic or
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
  both. Linked worktrees read the guide from the main tracker. A clone with no
  tracker uses the `--print` fallback without a false hand-edit notice; once the
  tracker exists, a missing or edited guide reads as stale and `init` writes it again.
  The link ends in a hash of the guide it points at, so a guide another version of
  moai wrote reads as that binary's, not as a hand edit — rebuild before you plant it again
- **`hook` needs the hooks installed**, so `init` installs them with it
  (`moai skill install --scope local`) and refuses `--no-skill` beside it. The
  first prompt of a session then carries the board and one line saying the usage
  is in the `moai` skill. That line appears in any checkout whose `AGENTS.md` has
  no moai block, and only there
- **Nothing records which one you picked.** Run again, `init` reads it from the
  block — a link means `file`, any other moai block means `block`, and that block is
  kept current even in a tracker kept out of git. These checks run on the first
  initialization too, so a clone keeps an existing link. With no block, installed
  moai hooks mean `hook`, and a plain rerun leaves those hooks installed without
  calling the installer again. Otherwise a tracker kept out of git is left without
  a block and a committed one gets the block
- **Switching from `file` to `block` takes the guide file away.** Once `AGENTS.md`
  holds the whole block, `moai init --guide block` removes `.moai/guide.md` and says
  so. A file there that does not open the way the moai guide does is yours: `init`
  leaves it and says that too
- **An `AGENTS.md` that links to a file `init` appends to gets no block.** With
  `AGENTS.md -> .gitattributes` or `-> .gitignore`, planting the block would replace
  the rules `init` just wrote. `init` leaves `AGENTS.md` alone and names it, and
  `--check` names it as well — make `AGENTS.md` a regular file to get the block

## Plant the skills

    moai skill install                     Claude Code, just me (the default)
    moai skill install --scope project     Claude Code, the whole team through the committed settings
    moai skill install --agent codex       Codex: the committed .agents/skills/ and .codex/hooks.json
    moai skill install --agent antigravity Antigravity: the same .agents/skills/ and .agents/hooks.json
    moai skill install --agent auto        whichever of claude, codex and agy is on PATH
    moai skill install --with moai-tmux    plant an optional skill too (--without takes it out)
    moai skill install --with moai-saycode the same, for the Saycode one
    moai skill status                      what is planted where, and what differs

`--agent` names who the skills are for — `claude` (what you get when it is left
out), `codex`, `antigravity` or `auto` — and can be repeated.

- **Claude Code gets a plugin.** `skill install` writes it into
  `.claude/moai-plugin/` and registers it with `claude`; your `settings.json` is
  `claude`'s to write, and `--scope` picks where it registers. A Claude session
  that is already open keeps the old copy until you reopen it. Claude's hooks
  come with the plugin
- **Codex and Antigravity read the same `.agents/skills/`** in the repository, so
  naming either writes it for both. There is nothing to register — commit the
  directory and the team has the skills. `--scope` does not apply to them, and
  one line says so when you give it without Claude
- **Their hooks are each agent's own file** — `.codex/hooks.json` for Codex,
  `.agents/hooks.json` for Antigravity — so only the agent you name gets one.
  Commit them with the skills. **Codex runs a project's hooks only once a person
  trusts them:** open `/hooks` in a codex session in the repository and trust
  moai's, and again after an install that changed them. A hooks file moai did
  not write — one with your own hooks in it — is left exactly as it is, and one
  line says so: merge moai's into it by hand, or move it aside and install again.
  For Antigravity that includes moai's own group with a handler of yours added to
  it, or turned off (`"enabled": false`)
- **`auto` looks at PATH** for `claude`, `codex` and `agy`, says what it found,
  and plants for Claude when it finds none of them

It is safe to run again: files are only overwritten. The one thing it deletes is
a skill directory an earlier moai planted in the same tree and this one no longer
plants — `skills/moai-work/` (gone in 0.9.0) anywhere, and `skills/moai-supervise/`
in `.agents/skills/` (it is Claude Code's only) — and only while it holds nothing
but the files moai planted there. Their text is not compared, so a `SKILL.md` you
edited goes with it, the way an install overwrites one. A directory with a file of
yours in it is left, and one line names it. `moai skill uninstall` takes Claude's
registration away and leaves the files; with
`--agent codex` or `antigravity` it prints the `rm -r` lines for moai's skills
in `.agents/skills/`, and the `rm` line for that agent's hooks file when moai
wrote it, and deletes nothing. `moai skill status` shows Claude's registration
and names a retired skill's directory left in Claude's tree,
whether `.agents/skills/` holds this version's skills (a CRLF checkout counts as
the same text) and names a retired skill's directory left there, whether each hooks file is
this version's (or not moai's) — judged by the moai that file calls, the way the
plugin is, so a different build running `status` does not ask to plant again —
and whether `codex` and `agy` are on PATH — and exits 0 whatever it finds.

**One text serves every agent.** Both trees get the same `moai` and `moai-wiki`
skills. The steps only one agent has — entering a worktree, asking the person,
calling the review, changing the model, clearing the window, calling a skill,
stopping what a review left running — sit in a "Words per agent" table in the
`moai` skill, one column per agent, and each agent reads its own. A step an agent
does not have reads `—`: tell the person and go on. The supervisor skill, its
tmux, cmux and Saycode companions and the recovery skill, planted for Claude Code only,
name Claude Code's tools directly.

Four skills are always planted, and three more are yours to choose:

    moai              the tracker itself — what to pick up, issues, plans, backlog items
    moai-wiki         keeps this wiki in step with the work
    moai-supervise    Claude Code only: hands piled-up backlog items to the idle sessions of the repository
    moai-recover      Claude Code only: brings back the sessions of the repository that died
    moai-tmux         optional, Claude Code only: the supervisor's hands on the workers' tmux panes
    moai-cmux         optional, Claude Code only: the same on the workers' cmux tabs (cmux 0.65.0 or later)
    moai-saycode      optional, Claude Code only: the supervisor's hands on the workers' Saycode sessions

- **`moai`** is the tracker skill — what an agent reaches for instead of a
  to-do list of its own
- **`moai-wiki`** is followed by the window that did an epic, to fix the page
  that teaches what the epic changed; a person can also call it to sweep
  everything merged since the last release
- **`moai-supervise`** makes a Claude Code session the
  [supervisor](glossary.md#supervisor), which hands the
  [backlog items](glossary.md#backlog), one at a time, to the
  [workers](glossary.md#worker) idle on the same repository and takes their
  reports. It picks, sends and checks; it does not fix and it does not merge.
  It is planted in the Claude plugin only, since it talks through Claude Code's
  own messaging — [Hand work to idle sessions](#hand-work-to-idle-sessions).
  The worker's way of working is on
  [the workflow page](workflow.md#work-in-a-worktree)
- **`moai-recover`** is what you call when the sessions died together — a
  restart, an OOM kill — to bring them back where they stopped —
  [Bring back sessions that died](#bring-back-sessions-that-died)
- **`moai-tmux`** is what the supervisor loads when it runs inside tmux: it maps
  a worker's `ListAgents` name to its pane and lets the supervisor label, clear
  and paste into that pane, and open new worker panes once you say yes —
  [In tmux](#in-tmux). It is planted only where you choose it
- **`moai-cmux`** is the same for a supervisor running inside a
  [cmux](https://github.com/manaflow-ai/cmux) tab: it maps a worker to its cmux
  surface, labels the tab, clears and pastes, and opens new worker splits once
  you say yes — [In cmux](#in-cmux). It is planted only where you choose it
- **`moai-saycode`** is what the supervisor loads when it runs inside a Saycode
  session: it lists the workers through `happy agent`, sends their work as a
  prompt, clears a reported worker, reads a stalled one, and opens new worker
  sessions once you say yes — [In Saycode](#in-saycode). It is planted only
  where you choose it

### Optional skills

An optional skill is planted only where you ask for it — today that is
`moai-tmux`, for a person who runs the supervisor inside tmux, `moai-cmux`,
for one who runs it inside cmux, and `moai-saycode`, for one who runs it inside
a Saycode session. Each takes the same lines with its own name.

    moai skill install --with moai-tmux       plant it
    moai skill install --without moai-tmux    take it out again
    moai skill uninstall --only moai-tmux     the same, under the uninstall name
    moai init --with moai-tmux                plant it with the rest on the first init
    moai skill install --with moai-cmux       the cmux one, the same way

- **Nothing writes the choice down.** A skill planted in the tree is the answer.
  So a plain `moai skill install` refreshes the optional skills already planted
  and plants no new one, and a first install plants none
- **The first `moai init` in a terminal offers a row per optional skill** under
  the hooks and skills row. A row starts checked when the skill is already
  planted, or when your shell says you use it (`$TMUX` set, for `moai-tmux`;
  `$CMUX_SURFACE_ID` set, for `moai-cmux`; `SAYCODE_AGENT_ENV` set, for
  `moai-saycode`).
  Where nothing is asked — a script, an agent, `--yes` — only what `--with`
  names is planted, never what the shell suggests
- **`uninstall --only` keeps the registration.** It removes that skill's
  directory (only while it holds nothing but moai's files) and brings Claude's
  registration up to the new version where it already stands; where none stands
  it changes the files and registers nothing. A tree moai has not planted
  stays as it is — no plugin, no `.agents/skills/`, no hooks file is made
- **A skill you left out is not drift.** The notice that the planted skills
  differ from this moai's does not count an optional skill you did not plant

## Open a session for each agent

The moai binary never starts a session, and nothing runs headless. Open each one
interactively in the root of the main checkout (inside tmux or cmux the supervisor may
open more Claude Code panes for you, once you say yes — [In tmux](#in-tmux),
[In cmux](#in-cmux)), after `moai skill install`
planted that agent's skills and hooks. Each session asks its person before it
acts, the way it always does — none is opened in a mode that skips the asking,
with one exception you are told of before you say yes: a worker session the
supervisor opens through Saycode, which Saycode starts with permission prompts
bypassed ([In Saycode](#in-saycode)).

The root, not a worktree: a session goes into its own worktree under
`.worktrees/` by itself, and that directory sits inside the root, so whatever
the root was given — Codex's trust, Antigravity's workspace — covers it too.

| | Claude Code | Codex | Antigravity |
|---|---|---|---|
| Open it in the root | `claude` | `codex` | `agy` |
| Plant | `moai skill install` | `moai skill install --agent codex` | `moai skill install --agent antigravity` |
| Skills | the plugin in `.claude/moai-plugin/` | `.agents/skills/` | `.agents/skills/` |
| Hooks | in the plugin | `.codex/hooks.json`, trusted once in `/hooks` | `.agents/hooks.json` |
| Supervisor | `/moai-supervise` | none | none |

### Claude Code

    claude

- **Open it the ordinary way**, not with `--dangerously-skip-permissions`
- **A session opened before the install** keeps the hooks it started with —
  older ones, or none — until it is reopened

### Codex

    codex

- **Trust the repository** when Codex asks. It reads a project's own `.codex/`
  only in a folder it trusts
- **Trust moai's hooks in `/hooks`** once, and again after an install that
  changed them. Before the first trust Codex runs none of them, and the five
  rules are words only. Codex keeps that trust per handler, by a hash of each
  one, so after an install that changed some of them the handlers it left alone
  keep running and the changed ones stop until you trust them again — nothing
  says so, and `moai skill status` does not see the trust

**Keep the sandbox where it stands.** Codex runs commands in a sandbox of its
own, and a machine where that works keeps it. `codex sandbox -- true` tells you:
where the sandbox cannot stand — a container whose AppArmor stops bubblewrap
from mounting, say — it fails with `bwrap: Failed to make / slave: Permission
denied`. On such a machine this repository runs Codex without the sandbox,
through a project file that stays out of git:

    # .codex/config.toml
    sandbox_mode = "danger-full-access"

and one line, `/.codex/config.toml`, in `.git/info/exclude`. **Do not commit
it**: anyone who clones the repository and says yes to Codex's trust question
would run with the sandbox off, and the question does not say so.
`.codex/hooks.json` in the same directory is committed — the two part ways
there. Only the sandbox goes: the approval setting is left at Codex's default.

### Antigravity

    agy

- **Open it the ordinary way**, not with `--dangerously-skip-permissions`. It
  asks its person before a command, like the other two
- **Pick the model as you open it** (`agy --model <model>`). The skills have no
  step for changing it later
- **Its hooks are `.agents/hooks.json`.** Antigravity has no session start and
  no prompt event, so the first model call of a turn stands in for both

## What the hooks do

The hooks catch the same few places in each agent's session. Each calls
`moai hook`, which nobody runs by hand.

| When | What happens |
|---|---|
| The session starts | A baseline of the warnings is written. After a compaction, what the session was holding is loaded back into it |
| A person sends a prompt | The `moai status` board is loaded, once per session |
| Before a tool call | The five rules below are checked |
| The turn ends | If the session still holds work, the turn is held once and asks for a [`Next:` note](glossary.md#next-note) for whoever comes after; it is also held when the [warnings](glossary.md#warning) grew. That question has a fixed room: open reviews tied to the held work, and held rows that would close their epic if deferred, get it first; the other held rows fill what is left from the top, those that do not fit are left out whole, and one more line counts them and names `moai prime`, which lists everything held |

**The hook never fails the session.** Whatever goes wrong inside it, it exits 0,
and the only thing it refuses is the one tool call that broke a rule. A person
typing `moai` in a terminal never passes through it.

**Each agent has its own shapes, the rules are one.** The hooks tell `moai hook`
whose they are (`--dialect`), and what differs is only what goes in and out:

- **Codex** sends hooks only for its shell, `apply_patch` and MCP calls — a
  patch is checked file by file, whether it came through the `apply_patch` tool
  or was typed as `apply_patch <<'EOF'` in the shell, and one file that breaks a
  rule refuses the patch
- **Antigravity** has no prompt event, so the first model call of each turn
  loads the board; it has no session start either, so the baseline is written
  there too. A line the rules only add to a tool call — fork 1's second
  question, say — has nowhere to stand in its answer and is not shown

**Where no hook stands, the rules are words only** — an agent without them,
Codex before the trust in `/hooks`, a tool call that sends none. Nothing is
refused there; the `AGENTS.md` block asks the agent to keep the five itself,
rules 4 and 5 most of all.

## The five rules

A refusal opens with the rule's number and name, and hands over the command that
gets through — run it as given. None of them waits on a person except rule 5.

1. **New issues stay inside what you picked up.** While an agent holds work (its
   [focus](glossary.md#focus)), a `moai add` must land in the same
   [epic](glossary.md#epic) (`-e <epic>`) or under the held issue
   (`--parent <id>`). Something for later goes in as `moai backlog add`, which this
   rule never stops; nor does it stop a whole plan created with `moai add --from`
2. **Pick something up before you change the repository.** A file edit
   (`Edit`, `Write`, Codex's `apply_patch`, Antigravity's file-writing tools) or
   a shell write (`>`, `>>`, `sed -i`, `tee`) to a file in the checkout needs a
   held issue — `moai mv <id> in_progress`, or `moai add` first if it was not in
   the plan. Not counted: `.moai/`, `.claude/`, `.agents/`, `.codex/`,
   `.worktrees/`, `.git/`, `target/`, `node_modules/`, and anything outside the
   repository
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

## Hand work to idle sessions

moai carries no messaging between agents. The supervisor is a Claude Code
session, and it talks to the others with Claude Code's own tools — `ListAgents`
to see who is here, `SendMessage` to hand over work and to report back. Codex
and Antigravity have no supervisor; their sessions pick their own work with
`moai ready`.

1. **Open the sessions.** Every Claude Code session a person opened in the
   repository's root on this machine that sits idle in `ListAgents` is a
   [worker](glossary.md#worker) — a Remote Control or cloud session cannot read
   the steps file the supervisor names. `ListAgents` shows no directory, so the
   supervisor knows a session of this repository by its name, which Claude Code
   slugs from the directory it was opened in (`moa-issue-bc` for `moa-issue`,
   `tvshop-updater-ca` for `tvshop_updater`) — a session you renamed is not
   counted. A name is only a candidate: a session of another repository whose
   name happens to start the same way refuses the work when `Root:` is not its
   repository. Nothing registers it, and nobody is asked which ones
2. **Make the supervisor — one per repository.** In one of them, call
   `/moai-supervise`; it asks you first whether another window already runs it,
   and a supervisor that is sent work refuses it. It picks
   backlog items that do not collide with the work open, and finds the workers
   in `ListAgents`: every idle session of this repository except itself
3. **The message carries the assignment and names the steps.** `SendMessage` to
   the worker opens with a line naming the work and telling it to read the
   [brief](glossary.md#brief) — the worker's numbered steps, a file of the
   supervisor skill (`references/worker.md`) given by its absolute path — and
   from which step to follow it. The worker is a session of the same repository
   and loaded the same plugin, so the file is there for it. The lines after that
   fill this assignment — the backlog, the model and difficulty picked for it,
   the work running alongside, the root branch and the base branch, the
   milestone, the root and whether the person is away. The supervisor
   sends it with `notify_when_idle` and waits for the report; it does not poll
   `ListAgents`
4. **The worker does the work in a worktree** — unfolds the backlog into an epic,
   picks the members up, works in `<root>/.worktrees/<epic>`, has the epic
   reviewed with `/code-review` inside its own session, merges, closes, leaves a
   `Next:` note and, last of all, reports with `SendMessage` to the supervisor
   that sent the work — if that send fails because the supervisor restarted, it
   leaves the report as a `report:` note on the epic, and a supervisor that
   starts or resumes reads those before it waits. Then it tells its person whether the window can be
   cleared now, and ends its turn; the next message wakes it
5. **The supervisor checks the report** — the merge is on that work's base branch, the
   epic is done, the worktree is gone — and sends the next backlog item

**The review runs inside the worker's own session.** It never starts another
agent program for it.

**Outside tmux and cmux, clearing a window is the person's.** The context lives
in the tracker, so a worker may be cleared (`/clear`) between tasks — its report
says when that is safe and when it is not. Without tmux or cmux the supervisor
never types into a window.

**What the messages do not do.** A session in a different permission mode
keeps an incoming message for its person's approval; a subagent's message goes
out under its parent session's address; an `@path` in a message attaches
nothing, so the message names the steps file by its absolute path and the
worker reads it.

**Stalled work** — a member picked up with no live session working it — the
supervisor looks for before any new backlog, and it reads it from the tracker,
not from `ListAgents`. The `stranded` warnings in `moai status --json` name rows
picked up while no live worktree holds them (a row picked up less than an hour
ago does not show), and the `Place` line of `moai show <id>` says where each one
stands. Only Place `none` is handed on to an idle worker; a path and `not
showing yet` are left alone. A place is read from the worktree's name and the
pick-up mark `moai mv` leaves where it was typed — never from a sibling's
snapshot — so a row picked up in the root and carried into a worktree whose
name is not its id shows `none`. `stranded` cannot tell a gone worktree from
work done in the root with no worktree, so while a `busy` session of the
repository stands in `ListAgents` the supervisor asks what it holds before
handing the row on. A worktree that is still there and whose name or mark points
at the row never shows under `stranded`:
its session has not ended because it reads idle — its person may be answering
it — nor because its name is gone, since a worker restarted with
`claude --resume` comes back under a new session name. In both cases the
supervisor waits for the person to say that window has ended. Whether stalled
work is carried on or put down is the person's call.

**Work inside a milestone has a branch of its own** (moai-nvju). The base
branch the message carries is the root branch for work outside every milestone,
and `milestone/<milestone id>` for work inside a live one — even one that has
not started running yet. That branch is checked out in a long-lived worktree,
`.worktrees/milestone-<milestone id>`, which the worker handed the milestone's
first epic raises from the root branch when it is missing. Each epic branches
from it and merges into it — the worker runs that merge, and the `branch -d`
after it, from the root as `git -C .worktrees/milestone-<milestone id> …` — and
the supervisor checks a report's merge on that branch, not on the root branch.
The root branch takes the milestone branch in once, at the release; until then
a milestone's finished work does not stand on the root branch. Branches go in
with `git merge --no-ff` as they stand, never rebased or squashed.

**When the person steps away**, they tell the supervisor, and its messages say
`Person: away`. The worker then settles a design question by its own
recommendation and leaves `Decided alone: …` on the issue instead of waiting,
and stops at anything that cannot be undone.

### In tmux

When the supervisor runs inside tmux (`$TMUX` is set) and the `moai-tmux`
skill is planted ([optional skills](#optional-skills) — `moai skill install
--with moai-tmux`), it also loads that skill and works on the workers' panes of
your own tmux server (moai-u99i). Without tmux, or without the skill, nothing of
this happens.

- **Which pane is which worker.** Claude Code keeps a record per process under
  `~/.claude/sessions/`; the skill reads the live ones (the pid alive and its
  start time matching) and pairs each `ListAgents` name with its pane `%N`.
  Every tmux call names that pane
- **A label on the pane.** When it sends work, it writes `<worker> <id>` into the
  pane option `@moai`, and removes it once the report is checked. Once a round
  it asks whether you want the labels on the pane borders; on a yes it puts
  `#{@moai}` in front of the running server's `pane-border-format`. Your tmux
  config file is never edited
- **Clearing a reported worker.** After the report is checked, while the worker
  reads idle and its input box is empty, it types `/clear` into that pane before
  sending the next work
- **A message that did not arrive.** When `SendMessage` fails, it pastes the
  message into the worker's empty input box and tells you. A message held for
  your approval is not pasted — it waits for you, and it tells you so
- **A stalled worker.** When a worker goes idle with no report, it reads the
  pane once and tells you what stands there — a permission prompt, a question,
  an error. It never answers in your place
- **No idle worker.** It asks you whether to open new worker panes; on a yes it
  splits a pane running `claude --model <model>` in the root — an ordinary
  interactive session you can see and type into

**If anything stands in a worker's input box, or you are scrolling that pane
(copy mode), it does not type** — it tells you.
It never kills a pane, a session or the server, and never runs `claude -p` or a
`--dangerously-*` flag.

- **A session Saycode's daemon started has no pane, and a pane two live
  sessions name is left alone.** Such a session carries in its record the pane
  the daemon was started from, so the skill reads that pane as nobody's and
  types nothing into it. A Saycode session is driven through `moai-saycode`
  instead

**On a Mac** the same works: the session records there carry the process start
time in another form (`ps -o lstart`), and a pane is matched to a session by its
terminal (`#{pane_tty}`), never by reading another process's environment
(moai-p5sz.7q3).

### In cmux

When the supervisor runs inside a [cmux](https://github.com/manaflow-ai/cmux)
tab (`$CMUX_SURFACE_ID` is set and `$TMUX` is not) and the `moai-cmux` skill is
planted (`moai skill install --with moai-cmux`), it loads that skill and works on
the workers' tabs of your cmux (moai-p5sz). Running tmux inside a cmux tab is
tmux's case — what is typed there goes to tmux. **It needs cmux 0.65.0 or
later**: the skill checks `cmux capabilities` once a round, and on an older cmux
it tells you and goes on as if it were not there. Updating restarts cmux and
every session in it, so when to update is yours.

- **Which tab is which worker.** The same records and the same session map; in
  cmux the map asks `cmux top` which surface each live session runs in. Every
  cmux call names that surface by its UUID, `--surface <UUID>`
- **A label on the tab.** Once a round it asks whether worker tabs should carry
  labels; on a yes it names the tab `<worker> <id>` (`cmux rpc tab.action`,
  action `rename`), and takes the name off once the report is checked (action
  `clear_name`). Claude Code's own title does not overwrite such a name. cmux
  cannot tell a name you gave a tab from Claude Code's title, so the question
  says a label replaces it; tell the supervisor which tab names are yours and
  those tabs get none. A label comes off only while the tab still reads it —
  a name you gave the tab meanwhile stays — and no workspace is renamed
- **Clearing a reported worker.** The same three conditions as in tmux, read
  from cmux's own view of the input box (`surface.input_state`) and from the
  session map, which has to show that worker still alive in that tab — a tab
  whose Claude Code ended can still show its last box; then it sends `/clear`
  and looks once
- **A message that did not arrive.** It pastes the message as one block with
  `cmux paste --submit`, after the same look at the box. cmux also refuses to
  type over someone's words in a Claude Code box its own Claude hook knows;
  that refusal is never forced
- **A stalled worker.** It reads the tab once (`cmux read-screen`) and tells you
  what stands there
- **No idle worker.** On your yes it opens a split beside its own tab running
  `cd <root> && claude --model <model>`, without taking focus, and evens the
  splits out. When cmux has no room for another pane it stops and tells you

It never closes a tab, a workspace or a window, never moves focus, and never
passes `--force`.

### In Saycode

When the supervisor runs inside a Saycode session (`SAYCODE_AGENT_ENV` is set,
and `happy agent whoami` answers) and the `moai-saycode` skill is planted
([optional skills](#optional-skills) — `moai skill install --with
moai-saycode`), it drives the workers Saycode lists through `happy agent`
(moai-l244). A Saycode session often runs inside tmux or cmux too; then Saycode
comes first, and `moai-tmux` or `moai-cmux` is only for a worker Saycode does
not list and for the pane labels.

- **Which session is which worker.** `happy agent ls --status` lists the
  sessions standing in the root or a worktree, with their state and what they
  wait on. The Saycode id is the key — a `/clear` changes a worker's
  `ListAgents` name, not its Saycode id. Nothing pairs the two: the session map
  marks a session Saycode started (its record's entrypoint, `remote_mobile`),
  and the supervisor leaves those out of the `ListAgents` workers it counts, so
  no worker is counted twice. A report still comes by `SendMessage`; the
  supervisor ties it to the work by the issue id it carries and the `Sent:`
  note (`saycode <id>`)
- **Sending work.** It starts `happy agent wait … --until turn-end` in the
  background and then sends the assignment with `happy agent prompt` (never
  `--wait`, which reports a failure though the message arrived). The wait ending
  is the notice that the turn ended; the report still comes by `SendMessage`
- **Clearing a reported worker.** After the report is checked, while the worker
  reads `idle` with nothing pending and you have not said you are talking to
  it, it sends `/clear` as a prompt
- **A stalled worker.** A worker in `waiting-input` — a question, a plan
  approval, a permission request — is named to you with what it waits for. It
  never answers in your place
- **No idle worker.** It asks you whether to open new worker sessions, and says
  each one runs with permission prompts bypassed (Saycode starts it with
  `--dangerously-skip-permissions`) and uses your spawn budget. The agent, model
  and effort are yours — a Codex or Gemini worker too — checked against what
  Saycode offers; nothing you left out is filled in. It opens them in the root,
  without Saycode's own worktree, and they show in Saycode's session list

It never `steer`s or `stop`s a worker unless you ask, and never prompts a
session that is still responding or waiting on you.

### Bring back sessions that died

When the supervisor and its workers died together — the machine restarted, the
container was OOM-killed — open one Claude Code session in the root and ask it
to bring them back ("recover the sessions", "되살려"). It loads `moai-recover`
(moai-uqf7); your asking is the yes.

- **Which died.** It reads the same records under `~/.claude/sessions/` and
  takes the dead ones standing in the root or a worktree, leaving out any that
  is already back and older crashes you did not name. When several stand in one
  directory it asks you once which to bring back
- **What each was doing.** From each session's transcript under
  `~/.claude/projects/` it reads its role (supervisor or worker), the issue it
  held and what it was waiting for, adds the uncommitted files and the issue's
  `Next:` note, and shows you one table
- **What died with it.** A background subagent or command that never reported
  back, and a `target` link whose `/tmp/cargo-target/<name>` was wiped — it
  makes that directory again; the build output is gone, so the session rebuilds
- **Bringing them back, workers first and the supervisor last.** Inside tmux it
  splits a pane per session running `claude --resume <id>` and pastes a short
  note on what happened once the input box is empty; the supervisor is told its
  workers' names may have changed. Inside cmux (0.65.0 or later) it does the
  same with a split tab per session, and when cmux has no room for another pane
  the sessions left get the lines to type. Outside both it prints, per session,
  `cd <dir> && claude --resume <id>` and the note to paste

A session Saycode ran is not resumed in a pane — that would run its
conversation outside Saycode. It tells such a session from its transcript,
finds its id as an `ended` row in `happy agent ls --status` and prints
`happy resume <saycode id>` for you to run in a terminal of your own, or you
reopen it from Saycode's session list. A worker that is one comes back before
the supervisor does.

It changes nothing in the sessions' work — no commit, no build, no `moai` write.

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

The moai binary never launches a session or runs one headless. A person opens
each one — or, inside tmux or cmux, says yes to the supervisor opening one — and the
session reads these the way that person would.

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
- **Codex runs none of moai's hooks.** Codex has not been told to trust them —
  open `/hooks` in a codex session in the repository and trust them. An install
  that changed them asks for that trust again
- **`moai status` says the planted skills or hooks differ from this moai's.**
  An older moai planted them — most often after upgrading. Run the line the
  notice names (`moai skill install`, with `--agent codex` or `antigravity`
  where those trees stand); it also removes what this moai no longer plants
- **`skill install` says a hooks file is not moai's.** It holds hooks someone
  else wrote, and moai will not overwrite them. Merge moai's entries into it by
  hand, or move it aside and install again
- **`skill install` printed two `claude plugin` lines and failed.** The
  registration did not go through — most often `claude` is not on `PATH`; run
  those lines where it is
- **`skill install` refused a path that "points at … outside", "is not a
  regular file" or "is not a directory".** A file or directory under
  `.claude/moai-plugin/`, `.agents/skills/` or a hooks file is a link that leaves
  the checkout (or lands in `.git`), a FIFO or device stands where a file goes,
  or a file or a dangling link stands where a directory has to be made (a
  `.codex` that is a file, say). moai checks every place before the first write,
  so nothing was written — not the other agents' files either — and `claude` was
  not asked to register. `--dry-run` stops with the same line. Replace the link
  with a real file or directory, or move aside what stands there, and install
  again
- **`init` or `skill install` refused a path that points into `.moai`.** A
  committed link (`AGENTS.md -> .moai/issues.jsonl`, a `SKILL.md`, a hooks file,
  or a directory such as `.agents -> .moai`) would land the write in the
  tracker. Nothing is written there and the link stays; `init` exits 0, `skill
  install` non-zero. Replace the link with a real file — see [Recovery](recovery.md#a-write-was-refused-because-a-link-leads-into-moai)
- **`skill install` said it did not read `.claude/settings.json`.** That
  committed file is a link out of the checkout, a FIFO, or unreadable, so the
  marketplaces an earlier moai declared there were not removed. Delete the
  marketplaces the line names from `extraKnownMarketplaces` yourself — in the
  file it links to, when it is a link
- **The board says the `AGENTS.md` block differs.** Another binary wrote it, or a
  person edited it. Build the moai you mean to run and call `moai init`
- **The agent keeps writing into a worktree's `.moai/`.** It does not need to: a
  `moai` run inside a linked worktree writes the main checkout's tracker by itself.
  See [the workflow page](workflow.md)
- **The supervisor finds no worker.** It counts only Claude Code sessions of
  this repository that read idle in `ListAgents` — by name: a session opened
  outside the root, or renamed, does not start with the repository's directory
  name. A session busy on its person's
  work is not one, nor is a Remote Control or cloud session, and Codex and
  Antigravity sessions never show there
- **A worker never answers.** Its session may run in another permission mode,
  where an incoming message waits for its person's approval — look at that
  window

Decided in: moai-2w0s moai-hxma moai-0zjo moai-nqdc moai-bl3x moai-gelm moai-tllo moai-mdzx moai-xs2h moai-h8tn moai-snyk moai-u5wr moai-b6cw moai-ew4o moai-dhxm moai-ml0d moai-nas5 moai-kxkw moai-dj4j moai-54yc moai-bkn4 moai-084j moai-keka moai-zynt moai-j9nf moai-jtvp moai-obxm moai-six5 moai-iu73 moai-r0x8

## Archive storage

`moai archive --dry-run` previews closed bundles and `moai archive` moves them
into `.moai/archive/<year>.jsonl`. It is an explicit maintenance command: normal
writes keep using the active snapshot. Milestones stay live. Statistics, search,
`show <id>`, `show --archived` and the explorer can read archive files too.

Reopening with `moai mv <id> todo --from done` restores only the selected row.
Its former bundle remains archived. `moai defer <id> --undo` on an archived row
brings it back the same way — a member reopened under an archived deferred epic
stays out of the plan until that epic is undone, and `mv` names it (moai-b6w3).
Archived IDs stay reserved, and `status`
reports IDs duplicated across storage files. `init` installs the archive merge
attributes alongside the active snapshot's rule (moai-fx9t).
