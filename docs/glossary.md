# Glossary

The words moai and these pages use, one heading per term, so any page can link
straight to a definition — `[epic](glossary.md#epic)`. Each entry says what the
word means here in a few lines and points at the page that teaches it. Flags are
in [the CLI reference](cli.md).

## Archive

Work that has stood in `done` for longer than `archive_days` (two weeks unless
`.moai/config.toml` says otherwise; `0` turns it off). Nothing is stored for it —
it is read off the column and the clock each time. The list, the board and
`moai show --all` leave it out; `moai show --archived` and `SPC v o` bring it
back. More in [the explorer](explorer.md#the-archive).

## Assignee

The person a row belongs to, written as a name and an email. Whoever creates a
row is its assignee unless `-a` names someone else; `-a none` leaves it unowned.
`moai ready` hands out only your own rows and sets the rest apart under `others`,
where `owner` says `theirs` or `unowned` — see [take over](#take-over).

## Brief

The numbered instructions a [supervisor](#supervisor) sends a
[worker](#worker): what to read first, which files the work running alongside
holds, and every step from [pick up](#pick-up) to merge and report. The
`moai-supervise` skill writes it.

## Column

Where a row stands — its `status` field. The columns are `todo`,
`in_progress`, `review` and `done` unless `statuses` in `.moai/config.toml`
lists others. The first column is work not started, the last is finished, and
every column between them is a started one. A [group](#group) has no column of
its own: it is read from its members. More in
[the workflow](workflow.md#pick-up-work).

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

## Group

An [epic](#epic) or a [milestone](#milestone). Belonging is inherited: a child
takes its parent's epic, and an issue takes its epic's milestone, so moving the
group moves its members along.

## Hook rules

The five rules the hooks that `moai skill install` plants into Claude check
before a tool call: stay inside your [focus](#focus), pick up before you change
the repository, a review is an issue, never kill the person's tmux server, and
ask before you [take over](#take-over). A refusal hands over the command that
gets through. More in [working with agents](agents.md#the-five-rules).

## Idea

A thought parked for later — a row of kind `idea`, off the board and out of
`moai ready`, so it does not blur the plan. `moai idea add` parks one (`SPC n`
in [the explorer](explorer.md#park-a-thought)), and
`moai idea promote <id> --from -` unfolds it into an epic
and issues and closes it. An idea is "not work yet"; [deferred](#deferred) is
"work, but not now".

## Journal

The append-only history under `.moai/journal/`, one file per writer named after
their email. It records creations, column moves, notes and removals — not field
edits — and is never read to work out where a row stands; that is the
[snapshot](#snapshot)'s job. Lose it and you lose history, not state. More in
[Recovery](recovery.md#the-files).

## Kind

What a row is: `issue` (the default, never written to the file), `epic`,
`milestone` or `idea`. Kind, [column](#column) and [deferred](#deferred) are
three separate questions — what it is, where it stands, and whether to look at
it now. A tag such as `bug` or `review` is not a kind.

## Member

A row that stands in a [group](#group), by its own field or by inheritance.
Members created by a [plan](#plan) carry the epic's id, `<epic>.<body>`, and
cannot leave that epic. A member left in the first column keeps the epic from
closing by itself.

## Milestone

A [group](#group) that carries dates — `--start` and `--due` — and is usually a
release. A deadline that has passed or falls due soon is one [warning](#warning)
line, nothing more. See [running milestone](#running-milestone).

## Model line

The note that names the AI that did the work, left before an issue is closed:
`model: <vendor>/<model> tokens=<count> (<grade> — <why>)`. Leave `tokens=` out
when the count is unknown, and write it on one id only. `--json` reads it back as
`work`, and `moai stats` adds it up. More in
[working with agents](agents.md#run-agents-without-a-person).

## `Next:` note

The note a session leaves on work it still holds when it stops —
`moai note <id> 'Next: <what comes next>'` — so the next session can carry on
from `moai show <id>`. The hook holds the end of a turn once to ask for it.

## Notice

A board line that informs without judging: ideas piling up, work deferred, a
milestone running, a merge driver not installed. It stands apart from the
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
one `- [p1] issue title #tag` line per issue — fed to `moai add --from -`
(`--dry-run` shows it first) or to `moai idea promote`. Show it to the person
once, before writing code. More in
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

## Supervisor

A session running the `moai-supervise` skill. It hands the [ideas](#idea) that
have piled up, one at a time, to the sessions idling on the repository, sends
each a [brief](#brief) and takes their reports. It picks, sends and checks; it
does not fix and it does not merge. More in
[working with agents](agents.md#plant-the-skills).

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
epic, a review stalled, too much started at once, work picked up and forgotten,
or a milestone falling due. Warnings block nothing and never change the exit code;
only broken data does that. Compare [notice](#notice).

## Worker

A session doing work a [supervisor](#supervisor) sent: it follows the
[brief](#brief), picks the work up, does it in a [worktree](#worktree), has it
reviewed, merges it and reports back.

## Worktree

A linked git worktree, one per piece of work, on its own branch, so sessions do
not pile commits onto one branch. The tracker stays in the main checkout; the
wiki pages ride the branch. More in
[the workflow](workflow.md#work-in-a-worktree).

Decided in: moai-tllo
