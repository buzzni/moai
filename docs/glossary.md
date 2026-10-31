# Glossary

The words moai and these pages use, one heading per term, so any page can link
straight to a definition — `[epic](glossary.md#epic)`. Each entry says what the
word means here in a few lines and points at the page that teaches it. Flags are
in [the CLI reference](cli.md).

## Archive

Work that has stood in `done` for longer than `archive_days` (two weeks unless
`.moai/config.toml` says otherwise; `0` turns eligibility off). `moai archive`
moves eligible closed epic bundles and standalone rows into yearly JSONL files
under `.moai/archive`; milestones stay live. A row under a parent, epic or milestone
waits until every one of those groups has also reached archive age. Board and ready count active work
and use archived rows as reference context, while `moai show --archived`, `moai show <id>` and `SPC v o`
read the archive. `moai mv` restores only the selected archived row, and so does
`moai defer <id> --undo` on an archived deferred epic. A live row
with a stale archive copy is repaired with `moai archive --drop <id>`. More in
[the explorer](explorer.md#the-archive).

## Assignee

The person a row belongs to, written as a name and an email. Whoever creates a
row is its assignee unless `-a` names someone else; `-a none` leaves it unowned.
`moai ready` hands out only your own rows and sets the rest apart under `others`,
where `owner` says `theirs` or `unowned` — see [take over](#take-over).

## Brief

The numbered steps a [worker](#worker) follows for work a
[supervisor](#supervisor) sent, from [pick up](#pick-up) to merge and report.
They stand in a file of the supervisor skill, `references/worker.md` — its rare branches
beside it in `references/worker-branches.md` — and a step
that goes differently for a subagent worker says so in a line of its own. Each
assignment message carries the lines for this assignment — the backlog, the
model and the effort, whether the worker is a subagent or a session, the work
running alongside, the root branch and the base branch (a
[milestone branch](#milestone-branch) for work inside a milestone), the milestone, the root and
whether the person is away — and one line telling the worker to read that file,
by its absolute path, and from which step to follow it.

## Column

Where a row stands — its `status` field. The columns are `todo`,
`in_progress`, `review` and `done` unless `statuses` in `.moai/config.toml`
lists others. The first column is work not started, the last is finished, and
every column between them is a started one. A [group](#group) has no column of
its own: it is read from its members. More in
[the workflow](workflow.md#pick-up-work).

## Deadline

The day a row is due — its own `due_on` field, set with `--due YYYY-MM-DD`, `--due +N`
(N days from today) or `@YYYY-MM-DD` at the end of a [plan](#plan) line (`@YYYY-MM-DD~YYYY-MM-DD` adds the start). Any row can
carry one, and it is asked about but never required. It cannot pass the deadline of
the [milestone](#milestone), [epic](#epic) or parent above it: such a write is refused,
and a row the one above overtook by being pulled in is a [warning](#warning),
`due_beyond_parent`. A deferred ancestor, or an ancestor under a deferred group,
sets no ceiling while deferred; undoing the deferral restores it, and plans now
beyond it stand as that same warning — the undo itself names none. It is not inherited — a member is bounded by its epic's deadline
without carrying it. A row's planned start (`--start`, `starts_on`) is held to the same ceiling: a start later than the deadline above it is refused, and one overtaken by pulling that deadline in is the same `due_beyond_parent` warning. A start earlier than the parent's start is never refused. `moai due apply` raises an open, not deferred issue's or backlog
item's priority as the deadline it follows nears — the earliest of its own and those of
the open, not deferred rows above it. Closed epics and milestones lend no priority
deadline; their columns come from their members, including archived members. An empty
group stays in the first column. One day left or less, today and past included, calls
for `p0`; two or less `p1`; seven or less `p2`. The command never lowers priority and
leaves a `Due-priority:` note. Deadline ceilings enforced on writes still count closed
ancestors. A group row is never raised itself; its members are. An open
issue or epic whose own deadline has passed is a [warning](#warning), `overdue`, and a priority
behind its deadline is a [notice](#notice) that names `moai due apply`.
More in
[the workflow](workflow.md#plan-something-bigger).

## Deferred

Work taken out of the plan for now, without changing its [column](#column) or
its [kind](#kind) — `moai defer <id> -m '<why>'`, and `--undo` brings the same
row back. It drops out of `moai ready`, the board and the [warnings](#warning).
Defer a group and what stands under it drops out too. Not the same as `done`:
never close what you decided not to do.

## Epic

A [group](#group) of issues that together deliver one thing. Pick one
[member](#member) up and the epic stands `in_progress`; finish them all and it
stands `done` — never `moai mv` the epic itself. An epic usually comes from a
[plan](#plan). More in [the workflow](workflow.md#plan-something-bigger).

## Focus

The work a session holds: rows assigned to you that have left the first
[column](#column) and are not closed yet. While you hold something, new issues
go inside it — rule 1 of the [hook rules](#hook-rules).

It is counted per session, not per person. A row a sibling worktree's name points
at is that worktree's, unless this worktree's name points at it as closely or this
session picked it up for certain. A row another session picked up last is not
counted, and nor — once another session has recorded a pick in the same tracker
within the last 12 hours — is a started row no session's hook recorded picking up:
moved in a terminal, by a session without the moai hook, or by a moai older than
0.29. `moai mv` and `moai add -s <started column>` record their own pick-ups for
the Claude Code session, including when the shell expands a variable. Such an
unrecorded row still counts when this worktree's name points at it — and so does
a row under it that this worktree's name points at more closely — or when it
sits under a row this session picked up or in that row's epic. Leaving rows out
only ever lets a call through
or holds the turn end less — it never adds a refusal. The session before a
`/clear` counts as another session; with no other session's pick recorded in that
window, an unrecorded row is still yours. A rule 1 or rule 3 refusal and the
turn-end question end with one line per held row on why this session holds it —
it picked the row up, this worktree's name points at it, or nobody recorded the
pick — when the lines fit. A rule 3 refusal with nothing held also names up to
three started review rows of yours it did not count, and why. When a rule 1, 2
or 3 refusal names a started row of yours that it did not count, it says why.
A row another session picked up last is left to that session; for an unrecorded
review the refusal gives a line that records the pick-up without moving it.
The reason follows the decision, including a cause inherited from an ancestor;
a pick marked “may not have run” is described that way. A recovery command is
offered only for a known column and a row in the tracker the command will write.

## Group

An [epic](#epic) or a [milestone](#milestone). Belonging is inherited: a child
takes its parent's epic, and an issue takes its epic's milestone, so moving the
group moves its members along.

## Hook rules

The five rules the hooks that `moai skill install` plants for each agent check
before a tool call: stay inside your [focus](#focus), pick up before you change
the repository, a review is an issue, never kill the person's tmux server, and
ask before you [take over](#take-over). A refusal hands over the command that
gets through. More in [working with agents](agents.md#the-five-rules).

## Backlog

A thought parked for later — a row of kind `backlog`, off the board and out of
`moai ready`, so it does not blur the plan. `moai backlog add` parks one (`SPC n`
in [the explorer](explorer.md#park-a-thought)), and
`moai backlog promote <id> --from -` unfolds it into an epic
and issues and closes it. The shared file still stores `kind: "idea"` so older
binaries can read it; issue JSON views report `kind: "backlog"`, except
`rm --json`, which returns the removed rows with the stored spelling.
The hidden `moai idea`
command alias remains for one release and is removed in v0.9.0.
A backlog item is "not work yet"; [deferred](#deferred) is
"work, but not now".

## Journal

The append-only history under `.moai/journal/`, one file per writer named after
their email. It records creations, column moves, notes and removals — not field
edits — and is never read to work out where a row stands; that is the
[snapshot](#snapshot)'s job. Lose it and you lose history, not state. More in
[Recovery](recovery.md#the-files).

## Kind

What a row is: `issue` (the default, never written to the file), `epic`,
`milestone` or `backlog`. Kind, [column](#column) and [deferred](#deferred) are
three separate questions — what it is, where it stands, and whether to look at
it now. A tag such as `bug` or `review` is not a kind.

## Member

A row that stands in a [group](#group), by its own field or by inheritance.
Members created by a [plan](#plan) carry the epic's id, `<epic>.<body>`, and
cannot leave that epic. A member left in the first column keeps the epic from
closing by itself.

## Milestone

A [group](#group) that is usually a release. Like any row it carries a start
date (`--start`) and a [deadline](#deadline). A milestone's deadline that has
passed or falls due soon is one [warning](#warning) line, nothing more. See [running milestone](#running-milestone).

## Milestone branch

`milestone/<milestone id>` — the branch a [milestone](#milestone)'s work
branches from and merges into when a [supervisor](#supervisor) hands it out,
checked out in a long-lived [worktree](#worktree) at
`.worktrees/milestone-<milestone id>`. The [worker](#worker) handed the
milestone's first epic raises it from the root branch; the root branch takes it
in once, at the release, and then it is removed. Work outside every milestone
uses the root branch. See [handing work to workers](agents.md#hand-work-to-workers).

## Model line

The note that names the AI that did the work, left before an issue is closed:
`model: <vendor>/<model> (<grade> — <why>)`, the model by its real name
(`opus-5.5`), not the `/model` alias. It carries no token count — what a window
or an agent shows is not what the work cost — though a line written with
`tokens=` before still reads. A subagent [worker](#worker)'s line is written by
the [supervisor](#supervisor). `--json` reads it back as
`work`, and `moai stats` counts the lines — folding the case of the vendor, model and
grade, so `GPT-6.1-Sol` and `gpt-6.1-sol` are one model shown in lower case,
while `work` keeps the spelling that was written. More in
[working with agents](agents.md#work-the-queue-from-a-session).

## `Next:` note

The note a session leaves on work it still holds when it stops —
`moai note <id> 'Next: <what comes next>'` — so the next session can carry on
from `moai show <id>`. The hook holds the end of a turn once to ask for it.

## Notice

A board line that informs without judging: backlog items piling up, work deferred, a
milestone running, a merge driver not installed, a priority behind its deadline. It
stands apart from the
[warnings](#warning) under `notices`. `moai wiki ls` uses the same word for a
page with conflict markers, a link that leads to no page, a link to a heading the
page does not have and an id the tracker does not hold.

## Pick up

Move a row out of the first [column](#column) to start it —
`moai mv <id> in_progress --from todo`. `--from` names the column you saw, so a
row someone else picked up meanwhile is not moved and you get `stale` instead.
The first pick-up stamps `started_at`. More in
[the workflow](workflow.md#pick-up-work).

## Plan

Markdown that creates an epic and its issues in one go — `# Epic title`, then
one `- [p1] issue title #tag` line per issue, any line ending in an optional
`@YYYY-MM-DD` [deadline](#deadline), `@YYYY-MM-DD~YYYY-MM-DD` (start to deadline) or
`@YYYY-MM-DD~` (start only) — fed to `moai add --from -`
(`--dry-run` shows it first) or to `moai backlog promote`. Show it to the person
once, before writing code. Claude Code can use `moai-plan` to interview the person and draft it on the plan model. More in
[the workflow](workflow.md#plan-something-bigger).

## Regression-of

A convention of this repository, not of moai: a bug that a merged epic caused
carries the line `Regression-of: <epic id>` in its body. Leave it out when you
do not know which epic. Nothing reads it yet. The rules are in `CLAUDE.md` — see
[the workflow](workflow.md#in-this-repository).

## Review issue

A review is an issue of its own, tagged `review` and created under the work it
looks at (`--parent`). Its body says what is being looked for and why, the
reviewer's words go in as a note, and closing it says what was taken in and what
was handed on. More in [the workflow](workflow.md#review).

## Running milestone

A [milestone](#milestone) with at least one [member](#member) in a started
[column](#column) — nothing opens it. While it runs, `moai ready` hands out its
work plus anything `p0`, and nothing else is pulled into it unless the person
says so.

## Snapshot

`.moai/issues.jsonl` — one JSON line per row, sorted by id. It is the truth:
every command reads where things stand from it, and every write rewrites it
whole under a lock. More in [Recovery](recovery.md#the-files).

## Writing detail

The person's `[writing] detail` selects `detailed` or `brief` for tracker writing.
`moai writing` shows the effective value and its source; see [how much agents write](agents.md#how-much-the-agents-write).

## Models table

`[models]` in your user config: which model each role runs on — `plan`,
`develop`, `complex`, `review`, `docs`, `bulk`, `codex` — with `review_grade`,
`effort`, `effort_complex` and `effort_plan` beside them. A key left out keeps its default.
`moai models` shows it and the [supervisor](#supervisor) reads
`moai models --json` once a round. A repository can recommend values in a
`[models]` table of its `.moai/config.toml`: they override the defaults, and your
user config overrides them — `moai models` marks such a value `repo`.
More in [working with agents](agents.md#the-models-are-yours).

## Supervisor

A Claude Code session running the `moai-supervise` skill. It hands the
[backlog items](#backlog) that have piled up, one at a time, to
[workers](#worker) — by default subagents it raises with `Agent` on the model
each role calls for ([models table](#models-table)), or, when the person wants to
watch their windows and for Codex, the sessions idle on the repository, sent a
message with `SendMessage`. The assignment names the [brief](#brief) to read, and
the reports come back the same way. It picks, sends and checks; it does not fix and it does
not merge. Inside tmux, where the optional `moai-tmux` skill is planted, it also
loads it to label, clear and paste into the workers' panes, and to open new ones
once the person says yes; inside cmux, `moai-cmux` does the same with tabs.
Inside a Saycode session, where the optional `moai-saycode` skill is planted, it
drives the workers through Saycode's `happy agent` the same way, and that comes
first. Codex and Antigravity have no supervisor skill; a Codex session can
still be a worker in a tab or pane. More in
[working with agents](agents.md#hand-work-to-workers).

## Take over

[Pick up](#pick-up) a row whose [assignee](#assignee) is someone else or
nobody — after asking the person:
`moai mv <id> in_progress --from todo --take -m '<who said yes>'`. You become
the assignee in the same write, and a `Taken-over:` note keeps whose it was.
More in [working with agents](agents.md#the-five-rules).

## Tracker

The `.moai/` directory: the [snapshot](#snapshot), the [journal](#journal) and
`config.toml`. A `moai` run inside a linked [worktree](#worktree) reads and
writes the main checkout's tracker.

## Warning

A board line on `moai status` about how the plan stands — such as issues with no
epic, a review stalled, too much started at once (the members of one epic count as
one), work picked up and forgotten, a milestone falling due, a row past its own
[deadline](#deadline) (`overdue`), or a row whose deadline passes the one above it. Warnings
block nothing and never change the exit code;
only broken data does that. Compare [notice](#notice).

## Worker

What does a piece of work the [supervisor](#supervisor) hands out — by default a
subagent it raises on the model the work's role calls for, otherwise any idle
Claude Code session of the repository other than the supervisor; nothing
registers it. It follows the [brief](#brief) — picks the work up, does it in a
[worktree](#worktree), has it reviewed, merges it — and reports back: a subagent
leaves the report on the epic and hands it back, a session sends it with
`SendMessage` and says when its window can be cleared.

A Codex session in a tmux pane or cmux tab can also be a worker. It reports by
epic note and stops at `Review-ready:` for the supervisor's review subagent.

## Worktree

A linked git worktree, one per piece of work, on its own branch, so sessions do
not pile commits onto one branch. The tracker stays in the main checkout; the
wiki pages ride the branch. More in
[the workflow](workflow.md#work-in-a-worktree).

Decided in: moai-tllo moai-snyk moai-u5wr moai-dhxm moai-kxkw moai-jtvp moai-bth3 moai-obxm moai-9tqo moai-uzhx moai-pyi4 moai-jdwk moai-lj8g moai-d1zn moai-ns7a moai-mkzo moai-w30o
