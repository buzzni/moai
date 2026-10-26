# Worker steps — the rare branches

This file stands beside `worker.md`, the steps every assignment takes; a line there names
the section here that your assignment needs. Read only that section.

## When the person is away

When the message says `Person: away`, decide by recommendation. A design question the notes
do not settle is not asked (step 4) — settle it the way you would have recommended, and write it
on the issue where the next person reads it

    moai note <id> 'Decided alone: <what you chose> — <why>, and what the other way was'

Nothing else waits on the person either. A model the window is not on is not asked for —
work on the window's model and say so in the reason of 9-1. A row that is someone else's or
nobody's (rule 5) is taken over only on a yes the message carries; without one, leave that row
and name it in the report.

**Stop at what cannot be undone** — deleting what is not yours, rewriting history someone
else has, a release, anything outside this repository — and report that instead of doing
it. When the person is back in the window, what they say overrides what you decided alone.

## A subdirectory project

When the message carries `Subdir:`, the root is not the top of the repository (a
subdirectory project in a monorepo). In step 3 the
worktree stands for the whole repository, so aim at the same subdirectory in it and work
there — standing at the worktree top, `moai` walks up and finds the root's
`.moai` to write, and the hook does not count edits under `.worktrees/`. `<subdir>` is that
relative path, filled in by the supervisor; with no `Subdir:` line the root **is** the top and this
section does not exist

    cd <root>/.worktrees/<epic>/<subdir> && …

## When the review stays refused

Step 7 sends you here: the review issue stands with its angle, you picked it up
(`moai -C <root> mv <review id> in_progress --from todo`), you ran what each refusal handed
you, and rule 3 still ties the review to a row that is not this epic's (its
`Why this session holds them:` lines, when it carries them, say which and why).
Do not work around the hook and do not start another agent program.
**Review by hand, in this session**: read the diff from the
merge-base yourself at the grade you picked, against the angle and the five places of step 7, fix
what you find in the working tree, and write your words to a file whose first line is

    Manual review: the review command was refused by rule 3 — <the refusal's first line>
That file is the review text 10 leaves on the review issue — without the line, the next
person reads your own sweep as what the review command found. The rest goes on as written: the
fixes in a `fix:` commit of their own, the close in 10, and the model line of 10 naming this
window's model with `manual` in its reason

## Carrying on stalled work

A session died holding a member of `<epic>`, the epic the message's first line names; that
member still stands picked up. Read how far it got (`moai show <epic>`, its history and
notes), then

- If the worktree is there, aim at it ("Stay in the root" in `worker.md`) — read how far it got with
  `git -C <root>/.worktrees/<epic> log <base branch>..HEAD` and `git -C <root>/.worktrees/<epic> status`
  (asked in the root, both read the root and show nothing of it), and carry on
- If it is not, raise it again from the root. If the branch survives, on that branch
  (`git worktree add .worktrees/<epic> worktree-<epic>`); if it does not,
  `git worktree add -b worktree-<epic> .worktrees/<epic> <base branch>` — inside a milestone,
  raise the milestone branch first if it is missing ("The milestone branch" in `worker.md`)
- **If the root is not the top of the repository** (a subdirectory project in a
  monorepo), aim at the same subdirectory inside the worktree and work there — standing
  at the top, `moai` finds and writes the root's `.moai`, and the hook does not count edits
  under `.worktrees/`. `<subdir>` is that relative path, filled in
  by the supervisor; with no `Subdir:` in the message, the root is the top and this step
  does not exist

      cd <root>/.worktrees/<epic>/<subdir> && …
- The member's column is already picked up — do not move it to another column, but **record
  it as this session's** with a move to the column it stands in, on a line of its own:
  `moai mv <member> <its column> --from <its column>` — nothing moves; the pick-up is
  written down for this session (`moai mv` records it itself). Standing in the root, the hook reads a member this session never picked up
  under `.worktrees/<epic>` as that worktree's, so without it you hold nothing: rule 1 stops
  fencing what you create, rule 3 refuses the review, and the turn's end does not hold the
  member for you. **If its assignee
  is not you** (`moai show <member>`), ask the person watching before you carry it on; on
  a yes, `moai mv <member> <its column> --from <its column> --take -m '<who said yes>'` — the
  column stays, the assignee becomes you, a note keeps whose it was, and the pick-up is
  written down the same way
- The note in 9-1 records this window's share only. Append `reclaimed work, the previous
  session's share is unknown` to the end of the reason — the previous session's model is
  written nowhere, and without it the whole member reads as this window's work
- Then go on with the steps from 4-1 to the end

## The review brief

In a subagent, 7 raises the review on this prompt — copy it whole and fill every slot.

    You run the epic-end review of <epic> — <title> — for the worker that raised you. You are a
    subagent of the same session, so the hook reads you as that worker.
    Worktree: <root>/.worktrees/<epic>, on branch worktree-<epic>
    Range: from <merge-base hash> (where the branch left <base branch>) to HEAD
    Review issue: <review id>, picked up already; its body is the angle — moai -C <root> show <review id>
    Grade: <grade>
    Root: <root>

    1. Run the review on that worktree: Skill(code-review, "<grade> --fix <root>/.worktrees/<epic>").
       It runs as a background fork of its own — wait for the notice that it ended, and do not
       hand back before it. If the hook refuses it (rule 3) and the review issue above is picked
       up, do not work around the hook: review the range by hand at that grade, fix in the working
       tree, and open the text of 4 with the line below — "If the review stays refused" in
       <steps file>, 7
       Manual review: the review command was refused by rule 3 — <the refusal's first line>
    2. Its fixes stay in the worktree's working tree, uncommitted. Do not commit, reset, rebase
       or amend there — the worker takes them in. If anything you started still runs, stop it
       with TaskStop
    3. Hold what came back against "Five places the review keeps finding" in <steps file>, and
       look yourself at any of the five the review left out
    4. Leave the reviewer's own words on the review issue — write them to a file in your
       scratchpad and run moai -C <root> note <review id> -b - < <that file>
       If the text runs past 64KB, summarize it — put `Summary: original <size>KB agent-<task-id>` on
       the first line, keep every finding's number and place, and shorten only the sentences. Leave fences and indentation alone
    5. Hand back with SubagentHandback. The first line is Model ran: anthropic/<your real model
       name> — the name in your system prompt, not an alias. When 1 ran the review by hand, the
       second line is Manual review. Then what was fixed (finding number, file) and what was left,
       by finding number
