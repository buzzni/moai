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

## Plant the skills

    moai skill install                     Claude Code, just me (the default)
    moai skill install --scope project     Claude Code, the whole team through the committed settings
    moai skill install --agent codex       Codex: the committed .agents/skills/ and .codex/hooks.json
    moai skill install --agent antigravity Antigravity: the same .agents/skills/ and .agents/hooks.json
    moai skill install --agent auto        whichever of claude, codex and agy is on PATH
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
wrote it, and deletes nothing. `moai skill status` shows Claude's registration,
whether `.agents/skills/` holds this version's skills, whether each hooks file is
this version's (or not moai's) — judged by the moai that file calls, the way the
plugin is, so a different build running `status` does not ask to plant again —
and whether `codex` and `agy` are on PATH — and exits 0 whatever it finds.

**One text serves every agent.** Both trees get the same `moai` and `moai-wiki`
skills. The steps only one agent has — entering a worktree, asking the person,
calling the review, changing the model, clearing the window, calling a skill,
stopping what a review left running — sit in a "Words per agent" table in the
`moai` skill, one column per agent, and each agent reads its own. A step an agent
does not have reads `—`: tell the person and go on. The supervisor skill, planted
for Claude Code only, names Claude Code's tools directly.

Three skills come with it:

    moai              the tracker itself — what to pick up, issues, plans, backlog items
    moai-wiki         keeps this wiki in step with the work
    moai-supervise    Claude Code only: hands piled-up backlog items to the idle sessions of the repository

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

## Open a session for each agent

moai never starts a session, and nothing runs headless. Open each one
interactively in the root of the main checkout, after `moai skill install`
planted that agent's skills and hooks. Each session asks its person before it
acts, the way it always does — none is opened in a mode that skips the asking.

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
   takes from the directory it was opened in (`moa-issue-bc` for `moa-issue`) —
   a session you renamed is not counted. Nothing registers it, and
   nobody is asked which ones
2. **Make the supervisor.** In one of them, call `/moai-supervise`. It picks
   backlog items that do not collide with the work open, and finds the workers
   in `ListAgents`: every idle session of this repository except itself
3. **The message carries the assignment and names the steps.** `SendMessage` to
   the worker opens with a line naming the work and telling it to read the
   [brief](glossary.md#brief) — the worker's numbered steps, a file of the
   supervisor skill (`references/worker.md`) given by its absolute path — and
   from which step to follow it. The worker is a session of the same repository
   and loaded the same plugin, so the file is there for it. The lines after that
   fill this assignment — the backlog, the model and difficulty picked for it,
   the work running alongside, the base branch, the milestone, the root and
   whether the person is away. The supervisor
   sends it with `notify_when_idle` and waits for the report; it does not poll
   `ListAgents`
4. **The worker does the work in a worktree** — unfolds the backlog into an epic,
   picks the members up, works in `<root>/.worktrees/<epic>`, has the epic
   reviewed with `/code-review` inside its own session, merges, closes, leaves a
   `Next:` note and, last of all, reports with `SendMessage` to the supervisor
   that sent the work. Then it tells its person whether the window can be
   cleared now, and ends its turn; the next message wakes it
5. **The supervisor checks the report** — the merge is on the base branch, the
   epic is done, the worktree is gone — and sends the next backlog item

**The review runs inside the worker's own session.** It never starts another
agent program for it.

**Clearing a window is the person's.** The context lives in the tracker, so a
worker may be cleared (`/clear`) between tasks — its report says when that is
safe and when it is not. The supervisor never types into a window.

**What the messages do not do.** A session in a different permission mode
keeps an incoming message for its person's approval; a subagent's message goes
out under its parent session's address; an `@path` in a message attaches
nothing, so the message names the steps file by its absolute path and the
worker reads it.

**Stalled work** — a member picked up with no live session working it — the
supervisor tells apart by the busy and idle rows of `ListAgents`.

**When the person steps away**, they tell the supervisor, and its messages say
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
- **Codex runs none of moai's hooks.** Codex has not been told to trust them —
  open `/hooks` in a codex session in the repository and trust them. An install
  that changed them asks for that trust again
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

Decided in: moai-2w0s moai-hxma moai-0zjo moai-nqdc moai-bl3x moai-gelm moai-tllo moai-mdzx moai-xs2h moai-h8tn moai-snyk moai-u5wr moai-b6cw moai-ew4o moai-dhxm moai-ml0d moai-nas5 moai-kxkw moai-dj4j moai-54yc moai-bkn4 moai-084j moai-keka moai-zynt moai-j9nf moai-jtvp moai-obxm

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
