# Worker steps

A supervisor — a Claude Code session running `moai-supervise` on this repository — sent you
a message that names this file. The message's lines are your assignment; this file is how to
do it. You know nothing else of the supervisor's conversation and need nothing else. **The
person comes first** — this window is theirs; when they speak, answer them.

## The assignment

The message's first line names the work and the step of this file to start from —
`from step 1` for a new backlog, `from step 2` for an epic already unfolded, or
`from "Carrying on stalled work"` for work a session left behind.

**This file is the path every assignment takes.** The rare branches stand in
`worker-branches.md` beside it — read that file too, the section a line here names, when your
assignment takes one: `Person: away` ("When the person is away"), a `Subdir:` line ("A
subdirectory project"), `from "Carrying on stalled work"`, a review rule 3 keeps refusing
("When the review stays refused"). The review brief is below.
The message's `from` is the supervisor — `<supervisor>` below; "tell the supervisor" is `SendMessage(to: <supervisor>, …)`.
A message the supervisor pasted into this window (`moai-tmux` or `moai-cmux`) or sent as a Saycode prompt
(`moai-saycode`) carries no `from`; its last line,
`from: <name>`, names the supervisor instead. Every
other line fills a slot the steps use; a line the supervisor adds beyond those — who already
said yes to taking over a row that is not yours, say — belongs to the assignment as well.

- `Model:` — `<model>` and `<difficulty>`.
  `Model:` is a suggestion picked by difficulty before anyone read the code; it is the model
  of a role in the person's table (`moai models --json`), and `Effort:` goes with it.
  **In a session** the model is changed by the person with `/model`, never by you — if this
  window is not on that model, ask the person watching the window to match it, and if it reads harder than it
  looked, raise it the same way to the model that pairs with the difficulty you just measured —
  the model of that role, not one step at a time. Handed `low` but it is `high`, the role is `complex`.
  **In a subagent** the model and the effort were fixed when you were raised, and nobody can
  change them. If it reads heavier than `Model:` says, stop before you pick anything up and hand
  back one line, `Model: raise to complex — <why>`; the supervisor raises a new worker on the `complex` model
  with the same message, its `Model:` and `Effort:` lines changed to that role's. What shows only
  after you picked up, you carry on and name in the report.
  Whether the epic-end review (7) is raised a step — only when `review_raise` allows it — is measured on this same rubric, member by member
- `Effort:` — `<effort>`, the reasoning effort that goes with the model. Like the model, a
  subagent was raised on it and a session's person sets it — you do not
- `Worker:` — `subagent` or `session`, how you were raised. A step that differs between the
  two says so in the step
- `Subagents:` — `<subagent ladder>`, the models you may raise subagents of your own on, one
  per role with its effort, for a piece of the work: `Agent(subagent_type: "general-purpose",
  model: <that role's model>, effort: <its effort>, …)`. If `Agent` refuses a model, do that
  piece yourself. Hand a piece to the role it reads as on the rubric in 5 — searching,
  summing up and the same mechanical fix many times to `bulk`; a piece of code to the role for
  development, or to `complex` when it touches the write path, concurrency, the storage format
  or hooks. Never to a lighter role than the piece reads as. What has to be decided stays in
  this window: a design question (4), what to take in from the review (7), the merge (8). A subagent of yours
  has no `EnterWorktree` — give it absolute paths under the worktree and the words of 4-1, 4-2
  and 4-3, and have it name its real model in what it hands back, for 9-1. The epic-end review
  stays as 7 says. `none` — do the work yourself
- `Writing:` — `detailed` or `brief`, the person's value the supervisor read this round. It
  stands for the board and for `moai writing --json` below: keep to it for tracker writing
  throughout the work, and pass it on to subagents of yours.

  Follow `Writing:` and the moai block's issue-writing rules; the moai skill's
  `references/commands.md` has the detailed and brief formats.

- `Work running alongside:` — the worktrees and work beside you, and the files they hold (4-3)
- `Root branch:` — `<root branch>`, the branch the root checkout stands on. The supervisor read
  it in the root; do not read it again — read inside a worktree, it gives that worktree's own branch
- `Base branch:` — `<base branch>`, the branch your worktree splits from and merges back into.
  Outside every milestone it is `<root branch>`; inside a milestone it is that milestone's own
  branch, `milestone/<milestone id>` ("The milestone branch")
- `Milestone:` — `<milestone>`, the release the epic hangs (1)
- `Due:` — `<due>`, the deadline the person gave for this work, `YYYY-MM-DD`, or `none`.
  The epic hangs it (1); `none` hangs nothing and never stands for a day you make up
- `Root:` — `<root>`, the root checkout's place (4-1)
- `Subdir:` — `<subdir>`, only for a subdirectory project in a monorepo (3). Without it the
  root is the top of the repository
- `Person:` — `here`, or `away` ("When the person is away" in `worker-branches.md`)
- `Worktree:` — the same words to every worker: you never go into the worktree, you aim at it
  ("Stay in the root")

Ask two things where the window stands now, before you move anywhere.
**If this window runs `moai-supervise` itself**, it is a supervisor, not a worker: do
nothing of it, reply to the message's `from` that you are a supervisor, and end the turn.
**If `Root:` is not this window's repository** — the window stands neither in it nor in one
of its worktrees — the supervisor took you for a worker by a name that only looks like its
repository's. Do nothing of it: reply to its `from` that you stand in another repository, and
end the turn.
The steps begin in the root the message names (`Root:`) — if this window stands anywhere
else in that repository, go there first: `cd "<root>"` from a subdirectory or from a worktree
a `cd` moved into, `ExitWorktree(keep)` from a worktree this session entered with
`EnterWorktree`.
A message that hands over no work is not work: if it asks something, answer it with
`SendMessage` to its `from`, and end the turn.

## Stay in the root

**Never call `EnterWorktree`**, in a session or subagent: every call into `.worktrees/` asks the person for approval, no allow rule turns it off, and subagents have no such tool. Aim each command at `<root>/.worktrees/<epic>` with `git -C` or `(cd "<root>/.worktrees/<epic>" && …)` and every Read and Edit at an absolute worktree path, including conflict resolution. A session's shell retains `cd`; parentheses keep it in the root. A subagent's Bash starts in the root each call.

Run the root branch check, merge, worktree removal and tracker commits in the root. If already in a worktree, return with `ExitWorktree(keep)` if you entered it that way, otherwise `cd "<root>"`. A merge run inside the worktree lands nothing; removing your current directory strands the shell. Rule 2 refuses root writes when all held work has worktrees and names where to write instead; never work around it.

## In a subagent

When `Worker:` says `subagent`, the supervisor raised you with `Agent` inside its own session,
and four things a session has are not yours. The rule for each stands here once; a step that
needs more says so in a line of its own, **In a subagent**.

- **No `EnterWorktree` or `ExitWorktree`.** A session does without them too ("Stay in the
  root"); for you they do not exist
- **No `AskUserQuestion`, and nobody watches you.** Where a step asks the person, hand back
  instead and end (4). `Person: away` still decides by recommendation —
  "When the person is away" in `worker-branches.md`
- **No `/model`.** The model and the effort you were raised on stay (the assignment's `Model:` line)
- **What you hand back is the report** (12) — `SubagentHandback`, and your turn ends with it.
  Where a step says to tell the supervisor and stop, that is your hand-back; where it says to
  tell the supervisor and go on, it goes in the report. A message that hands over no work is
  answered the same way
- **Your tool calls carry the supervisor's session.** The hook reads you two as one: your
  pick-ups are recorded as the supervisor's, its rules see what the supervisor's other workers
  hold, and the supervisor's turn-end holds what you hold

## Before the steps

Before you commit or merge in the root, **only check** that the root still stands on
`<root branch>` — run `git symbolic-ref -q HEAD` **in the root**. If it is not `refs/heads/<root branch>`
(detached, or someone switched the branch), do not run that commit or merge: tell the supervisor and
stop. A merge that lands on the wrong HEAD leaves no reference at all once `branch -d` runs.
Work inside a milestone merges in the milestone's worktree, not in the root, and that worktree
gets the same check before the merge ("The milestone branch").

**Ask it where you already are.** You stay in the root ("Stay in the root" — in a session,
as long as every `cd` into the worktree stood in parentheses), so the merge and the tracker
commits (8, 11) and this check are plain commands there.
Before a tracker commit in the root, look at `git status -- .moai/` first. The path keeps the
commit from sealing someone's open merge; the rows other sessions have not committed yet ride
along, and that is expected — they stand in the shared file already, and the next tracker
commit is where they go into history

**Some repositories keep the tracker out of git** — `.moai` in `.gitignore` or
`.git/info/exclude`. Ask once, in the root, before your first tracker commit — the one in 11,
or one an earlier step makes (8 can call for it)

    git -C "<root>" ls-files --error-unmatch -- .moai/issues.jsonl      a non-zero code: the tracker is not in git
On a non-zero code every tracker commit in these steps — each "commit with a path, as in 11" —
does not exist: the `moai` writes still land in the root's file, and there is nothing to commit.
Skip them and nothing else; the merge and the branches still go through git. A zero code is the
usual case, and every step stands as written

## The milestone branch

For a milestone base, read `worker-setup.md` — The milestone branch.

## The review brief

In a subagent, 7 raises the review on this prompt — copy it whole and fill every slot.
Create `<scratchpad>/<epic>/` first and save the filled prompt as
`<scratchpad>/<epic>/brief.md`. Pass that absolute path to the review subagent with
instructions to read the whole file before starting. Use this epic's directory for all
review scratch files; a shared `<scratchpad>/brief.md` lets parallel epics overwrite each
other's prompts.

    You run the epic-end review of <epic> — <title> — for the worker that raised you. You are a
    subagent of the same session, so the hook reads you as that worker.
    Worktree: <root>/.worktrees/<epic>, on branch worktree-<epic>
    Range: from <merge-base hash> (where the branch left <base branch>) to HEAD
    Review issue: <review id>, picked up already; its body is the angle — moai -C "<root>" show <review id>
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
    4. Leave the reviewer's own words on the review issue — write them to
       `<scratchpad>/<epic>/review.md`, beside this epic's `brief.md`, and run
       moai -C "<root>" note <review id> -b - < "<scratchpad>/<epic>/review.md"
       If the text runs past 64KB, summarize it — put `Summary: original <size>KB agent-<task-id>` on
       the first line, keep every finding's number and place, and shorten only the sentences. Leave fences and indentation alone
    5. Hand back with SubagentHandback. The first line is Model ran: anthropic/<your real model
       name> — the name in your system prompt, not an alias. When 1 ran the review by hand, the
       second line is Manual review. Then what was fixed (finding number, file) and what was left,
       by finding number

## The steps

1. For a backlog assignment, unfold it in the root as `worker-setup.md` step 1 says.
2. Pick the members up with `moai mv <member> in_progress --from todo` in the root.
   **Pass the column you saw** — this is a place where several sessions share one `.moai`,
   and overwriting a row picked up beside you means two of you do the same work. A non-zero
   code means it is taken, so leave that member and tell the supervisor.
   **Do not commit the pick-up.** Every session reads the root's file, not git, so the row
   shows beside you the moment it is written; the one tracker commit in 11 carries it.
   **A member that is someone else's, or nobody's, is asked about** — the hook refuses that
   pick-up (rule 5). Ask the person watching this window — a yes the message already carries
   counts; on a yes, run the line the refusal hands you (`--take -m '<who said yes>'`), on a
   no leave that member and tell the supervisor.
   **In a subagent** there is nobody to ask: without a yes in the message, leave that member
   and name it in the report. Handed an epic `from step 2`, measure first as `Model:` says,
   before you pick anything up
3. Right after 2, branch from the local <base branch> with
   `git worktree add -b worktree-<epic> .worktrees/<epic> <base branch>` from the root, and do
   not go in: from here on aim every command at `<root>/.worktrees/<epic>` ("Stay in the
   root"). If the repository's CLAUDE.md says what a new worktree gets (a link for the build
   output, say), do that first. The name is the unfolded epic's id, not the backlog's.
   Inside a milestone the milestone branch has to stand first — raise it if it is missing
   ("The milestone branch").
   Until the worktree stands, the other sessions in the root read this member as their own focus.
   **If the message carries `Subdir:`**, the root is a subdirectory of the repository — work
   in that subdirectory of the worktree, as "A subdirectory project" in `worker-branches.md` says.
4. Before you ask about a design decision, read the `Wiki` block of `moai show <member>` and
   `moai wiki search '<that place>'`; do not ask a person what the wiki already answers.
   Do not guess a design decision that is not in the notes — ask with `AskUserQuestion`;
   a person is watching the worker's window. With `Person: away` in the message, decide by
   recommendation instead ("When the person is away" in `worker-branches.md`).
   **In a subagent** there is no `AskUserQuestion`: hand back with `SubagentHandback`, its
   head `decision needed: <id>`, then the situation, options A and B, and your recommendation —
   and end. The supervisor asks the person and wakes you with the answer (`SendMessage`);
   carry on from where you stopped

4-1. **The tracker you edit is always the root's.** Follow the moai block's worktree tracker rule. `<root>` is the message's `Root:` — do not guess it from inside the worktree. Use `moai -C "<root>"` to make the target explicit; never set `MOAI_HERE`. Give review subagents the same rule. Put `-e <epic>` on backlog parked mid-epic so 7-1 finds it. If this worktree's `.moai` changed, undo it in the worktree with `git -C "<root>/.worktrees/<epic>" checkout -- .moai`; never undo the shared root tracker. If already committed, undo that commit too and park again from the root.

4-2. If testing tmux, follow `worker-tmux-tests.md` before starting a test server.

4-3. **If you would have to touch a file that work running alongside holds, do not fix it** —
   the files named by `Work running alongside` in the message, or files a sibling branch in
   `git worktree list` already changed
   (`git diff --name-only <base branch>...<sibling branch>`). When two of them change the
   same place, one waits for the other at the merge. If this epic cannot deliver what it
   promised without that, it is not a backlog item but a member — create it with
   `moai -C "<root>" add '<what>' -e <epic>`, leave it in the first column, and name it in 12
   as **a member left because the work beside it holds the file**, together with that other
   work. The supervisor sends it once that work is done. Do not defer it.
   **If the hook refuses that line** (rule 1), read the refusal's `Why this session holds them:`
   lines — they name the rows the hook reads as this session's, and why. That head comes in the
   screen's language, and a refusal that would run too long carries none. Do not take the
   `-e` or `--parent` it offers for another epic or another row, and do not work around it.
   Park it instead — a backlog item is free of rule 1, and `-e <epic>` lets 7-1 find it
   (`moai -C "<root>" backlog add '<what>' -e <epic>`), then name it in 12 in one line

    member refused: <backlog id> for <epic> — rule 1 read <ids> as this session's (<their why lines>)
   The supervisor makes it a member from its own window, with the line of 7-1. In 7-1 it is
   one of the backlog items to reclaim like any other — if the hook lets `promote` through
   there, reclaim it yourself and say so in that line instead

5. **Do not review member by member.** When one member is finished, run the tests, commit and
   move to the next — the review looks at the whole epic once, in 7, after every member is
   finished. One review is expensive; do not call it as many times as there are members. The
   cost of member 2 piling onto a bug in member 1 is paid in that one review.
   `low`·`medium`·`high` is the rubric the model in the message was picked on, and the same rubric measures
   the members when you pick the grade in 7.
   - `low` — text, comments, a one-line fix; behaviour unchanged
   - `medium` — a behaviour change inside one file, ringed by tests
   - `high` — several files, the write path, concurrency, the storage format, hooks; hard to undo
6. When the members' work is all done, pull <base branch> into the worktree, resolve the
   conflicts and run the tests. Fix things here, never in the root — the root is shared and
   stands on `<root branch>`; the hook refuses a write there only while everything you hold
   has its worktree standing ("Stay in the root"), and this line is what holds the rest.
   Inside a milestone that is the milestone branch, not `<root branch>` — the epics merged
   beside you are there
7. Review the whole epic once before merging — still this session, not another agent program. In Codex follow `worker-codex.md` step 7 instead. Read `review`, `review_grade` and `review_raise` from `moai models --json` and use `review_grade`. Only with `review_raise = "write-path"` raise it one step, once and capped at `max`, when a member touched the write path, concurrency, the storage format or hooks, or this worktree carries members of two epics (measured as one epic); `never` keeps the configured grade. Ask the person to put a session on the `review` model if it is not.

   Put the epic's `Review focus / 리뷰에서 볼 것` section (either heading) first in the angle, then development focus and the grade and reason. Use `<scratchpad>/<epic>/angle.md` and `-b -` for multiline or quoted angles; never share scratch directories. The range starts at `git -C "<root>/.worktrees/<epic>" merge-base <base branch> HEAD`, after pulling the base in 6. Create the review issue under the epic (rule 3):

       moai -C "<root>" add 'review — <what you are looking at>' -t review --parent <epic> -b '<what you are looking for and why>'

   In the same breath move finished members one id per call:

       moai -C "<root>" mv <member> review --from in_progress

   A member left in the first column (4-3, 7-1) answers with a non-zero code and stays — that is the pass-over, not a lost pick-up as in 2; one already in `review` stays too. moai answers in the screen's language, so read what it says: if it refuses `review` itself as not a column, leave the members in `in_progress`. Then **pick the review issue up**, and only then call the review:

       moai -C "<root>" mv <review id> in_progress --from todo

   In a session call `/code-review <grade> --fix <root>/.worktrees/<epic>`. In a subagent fill "The review brief" above, save it as `<scratchpad>/<epic>/brief.md` and raise it on the `review` model:

       Agent(subagent_type: "general-purpose", model: <review model>, description: "review <epic>", prompt: "Read <scratchpad>/<epic>/brief.md whole before you start, then follow it")

   It leaves its fixes uncommitted and its own words on the review issue. If a hook refuses, run its correction. **If the review stays refused** — picked up, every correction run, rule 3 still tying it to a row not this epic's — do not work around the hook and start no other agent program: review by hand as "When the review stays refused" in `worker-branches.md` says.

   **Five places the review keeps finding.** Add them to the angle and hold them against the actual review: a new item taking another's doc block; a test that cannot go red on a revert; setup duplicated across paths; comments or docs disagreeing with behavior; and a path newly opening a file it did not open before. The angle does not reach the review command by itself.

   While review runs do not touch, reset, rebase or amend the branch or worktree. Wait for it, stop anything it left running with `TaskStop` and inspect the worktree. Take fixes in a separate `fix:` commit; note handoffs with issue IDs. Merge with `--no-ff`, never squash or rebase: commit subjects identify issues and milestone merge hashes remain stable.

7-1. Before merging, inspect backlog parked mid-epic (`moai -C "<root>" show --type backlog -e <epic>`) and review handoffs. If the epic cannot deliver without one, it is an unfinished member, even if it waits on a decision or a sibling's file. Do not promote an already closed backlog. Unfold only `- issue` lines into the standing epic:

    moai -C "<root>" backlog promote <backlog id> -e <epic> --from -
   Leave reclaimed members in the first column, not deferred and not in this merge: review 7 did not see them. Name them in the `Next:` note (11); the session that closes the epic reviews it again.

7-2. If the epic ran in a milestone, sort handed-on work by the release bar. Bug-level work stays in the epic or another epic in that milestone; work the release can do without goes outside:

    moai -C "<root>" edit <that row> -e none --milestone none
   Both flags matter because the milestone is inherited. Use the existing `#bug` tag and `Regression-of:` test to judge bug-level; do not invent another tag or field.

7-3. If there is a CHANGELOG, check the upcoming release section and add this epic's entry in the worktree before merging. Aim the commit at `git -C "<root>/.worktrees/<epic>"`. Commit all of the epic's entries once after members and review fixes; if a later fix changes what ships, update its entry in that fix, not a second changelog commit. Nothing blocks an empty section.

    git commit -m "docs(changelog): <what this epic changed> (<epic>)" -- CHANGELOG.md

7-4. If there is a wiki (`moai wiki ls`), ask whether a person-facing key, command, flag, file, format or procedure changed. If so follow `moai-wiki` in the worktree and commit there before merging; aim wiki calls at `moai -C "<root>/.worktrees/<epic>"` because bare calls read the root, and both git lines below at `git -C "<root>/.worktrees/<epic>"`. `<wiki dir>` is `dir` in `moai wiki ls --json`. If not, write nothing; nothing checks this. If one rule could have stopped a review finding, follow Lessons in `moai-wiki` in the same commit.

    git add -- <wiki dir>
    git commit -m "docs(wiki): <what changed> (<epic>)" -- <wiki dir>

8. Merge from the root, where you stand ("Stay in the root").
   Before merging outside a milestone, check that the root stands on <root branch> — if it
   does not, do not merge: tell the supervisor

       git symbolic-ref -q HEAD                  it has to be refs/heads/<root branch>
   **Inside a milestone you merge in the milestone's worktree, not in the root** — check that
   worktree's HEAD instead, with the line in "The milestone branch", and run the merge and its
   abort below with `-C .worktrees/milestone-<milestone id>` after `git`, from the root.
   Merge **in one call**. Overlap with the workers beside you was split when the
   supervisor sent the work, and where it still collides, undo as below and resolve in the
   worktree — do not go looking for the other session to tell it. Do not use `--no-commit`.
   Without `--no-ff` it ends as a fast-forward and no merge commit stands

       git merge --no-ff worktree-<epic> -m "merge: …"
       git -C .worktrees/milestone-<milestone id> merge --no-ff worktree-<epic> -m "merge: …"     inside a milestone
   If the merge is refused over the root's uncommitted `.moai` — yours since 2 or another
   session's — take them in first with a commit with a path, as in 11, its subject
   `chore(tracker): take the root's rows in before <epic> merges`. If it stops on a conflict,
   do not resolve it where it stopped — undo with `git merge --abort`, aim at the worktree again
   and run again from 6
9. Once the merge has really landed, remove the worktree and the branch from the root with
   `git worktree remove .worktrees/<epic>` and `git branch -d worktree-<epic>`.
   **Inside a milestone delete the branch in the milestone's worktree** —
   `git -C .worktrees/milestone-<milestone id> branch -d worktree-<epic>`. `-d` asks whether
   the branch is merged into the HEAD it runs in, and `<root branch>` in the root does not
   hold this merge until the release, so from the root it refuses. Do not reach for `-D`.
   Leave the milestone's worktree standing

9-1. Before closing, leave one line per member **on what did this work** in this window —
   leaving out the members left in the first column by 7-1 and 4-3, which nobody did. Not the
   suggestion in the message but the model that **actually ran** in this window. The line below
   was filled in by the supervisor as a suggestion, so if you raised it, or the window was on
   a different model from the start, correct the model and the difficulty to the real ones and
   write why in the reason — the next person reads "what was put on work of this size" there.
   It is a note, not a field. The supervisor does not fill `<vendor>` — the vendor is
   `anthropic`, and the model is its real name (`opus-5.5`), not the `/model` alias — the
   `<model>` the supervisor filled in is an alias (`opus`), so write the real name even if you
   did not change models. **No token count**: the line carries no `tokens=` — what a window
   shows is not what the work cost, and a false number is worse than none.
   Quote free text with single quotes — inside double quotes the shell expands backticks and
   `$(…)` as commands. If the text itself contains a single quote, stream it from stdin with `-b -`

    moai note <member> 'model: <vendor>/<model> (<difficulty> — <why>)'
   **A subagent of yours (`Subagents:`) that did a share of a member gets a line of its own** on
   that member, the same form, by the real name it handed back and with `subagent` in the
   reason — a member can carry several lines, one per model that worked on it.
   **In a subagent, skip this step** — the supervisor writes these lines from your report's
   `Model ran:` and `Subagent ran:` lines (12)

10. Close them after that. **Run `moai mv <member> done` only once that merge has really
    landed** — closed before it, a merge that stops on a conflict leaves them done on work
    that is not in. It closes a member from `review`, where 7 stood it, and from
    `in_progress` where there is no `review` column alike. Do not close the
    members left in the first column by 7-1 and 4-3 — those members keep the epic open. While
    the worktree still stands, the hook reads what this session did not pick up as a sibling
    worktree's and cannot refuse such a review closed without `-m`. Close the review issue
    leaving what came out of it

        moai note <review id> -b - < <review text>   the reviewer's own words (summarize past 64KB)
        Could one rule have stopped any finding above? If so, write the lesson on the branch before it merges — `moai wiki search` it, then follow Lessons in `moai-wiki`
        moai mv <review id> done -m '<what you took in, what you handed on>'
    If the text runs past 64KB, summarize it — put `Summary: original <size>KB agent-<task-id>` on
    the first line, keep every finding's number and place, and shorten only the sentences. Leave fences and indentation alone

    Leave one line on the review issue too, naming the model the review ran on. In a session
    that is this window's model, by its real name — a review agent inherits the window's model —
    and `manual` joins the reason when the review ran by hand (7, or a review subagent's
    hand-back whose second line says `Manual review`)

        moai note <review id> 'model: anthropic/<model it ran> (<grade> — epic-end review)'
    **In a subagent** the review subagent already left the reviewer's words on the review
    issue, so write only your own call (`mv … done -m`) and that line —
    `<model it ran>` is the name after `anthropic/` on the first line of its hand-back, so the
    vendor stands once.

    **Then look for a member left without its line** — 9-1 is the step that gets skipped, and a
    member closed without it is counted as nobody's work. Every id this prints, other than the
    members left in the first column and the ones finished before this epic came to you, is a
    member 9-1 missed: write its line now. A row this window never worked on stays without a
    line — your model on it is a false line. In a subagent skip this — the members' lines are
    the supervisor's, from your report's `Model ran:`

        moai -C "<root>" show -e <epic> --all --json | jq -r '.[] | select(.work == []) | .id'
    Leave the tests passing in the root
11. Leave the line to take over from — `moai note <epic> 'Next: …'`. If anything is left (a
    background review, say), finish it before the note — the supervisor reads the note as this
    work being over; what you cannot finish, name in the report (12).
    Then take the tracker into the root with a commit with a path — **one** for the epic. It
    carries the pick-up of 2, the lines of 9-1, the closes of 10 and this note together, not one
    commit per step. A report left as a note in 12 — and, in a subagent, the `model:` lines the
    supervisor writes from it — come after it; the supervisor's own tracker commit carries those.
    When a step says "a commit with a path, as in 11", this is its shape. The root is shared by
    every session — a commit made while someone has a merge open (MERGE_HEAD) seals that merge
    with its own subject. So give the tracker commit a path. With a merge open git refuses it,
    so wait for that merge to finish and run it again

        git commit -m "chore(tracker): close <epic> with its merge" -- .moai/
12. Report to the supervisor, **last of all**, with `SendMessage(to: <supervisor>, message: …)`
    — `report: <epic>` at its head. It carries the merge hash,
    the unfolded epic's id, a line or two of summary, what you handed on and any new backlog,
    the members reclaimed in 7-1 and left in the first column,
    the members left in 4-3 because the work beside you held the file, with that other work
    named, the `member refused:` lines of 4-3,
    the wiki pages 7-4 changed — or that it changed none — and anything still running that 11
    could not finish. **A subagent worker adds a `Lessons:` piece** — the slug of each new
    lesson page and each `Lesson:` note that met an old one, or `Lessons: none` — so
    what the epic learned stands in the report as well as on the branch.
    **If that send fails** — the supervisor restarted, so its old name is gone — the report
    must not be lost: leave the same text, `report: <epic>` at its head, on the epic with
    `moai note <epic> -b -` — it stands in the root's file, where the next supervisor reads it,
    so it needs no commit of its own. Tell
    the person watching that the report is on the epic; the next supervisor reads it there.
    Then **say when the window can be cleared**, in one line to the person watching. The
    context lives in the tracker, not in the conversation: issue bodies, notes, review texts,
    commit messages. If you can see your own context usage, put that number in the line too.
    **Say the opposite in the same line** — not to clear while a review is running in the
    background, while a merge conflict is being resolved, while waiting on a person's answer,
    or after the supervisor's next message has arrived in this window. Clearing (`/clear`) then
    loses what is not yet moved into the tracker, or the message that arrived.
    Then end the turn. The supervisor's next message is the next work.
    **In a subagent** the report goes two ways, the note first: leave the text on the epic
    with `moai note <epic> -b -`, `report: <epic>` at its head — a supervisor that restarted
    has lost the notification and reads the note from the root's file; it needs no commit of
    its own. Then hand back the same text with `SubagentHandback`, and your turn ends.
    Its second line is `Model ran: anthropic/<your real model name>` — the name in your own
    system prompt (`opus-5.5`), not the alias in `Model:`; the supervisor writes 9-1's line
    from it. Below it goes one `Subagent ran: anthropic/<its real name> on <member>` line for each
    subagent of your own (`Subagents:`) that did a share of a member — 9-1's line of its own.
    Nothing about clearing a window is yours
