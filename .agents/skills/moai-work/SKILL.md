---
name: moai-work
description: Use when a person makes this window a worker for a moai supervisor — say hello, wait for a letter, do the work it hands over in a worktree, report it, and wait again. Triggers on "be a worker", "wait for work", "take work from the supervisor", "일꾼 해", "일 기다려", "감독 일 받아".
---

# moai-work — wait for work from a supervisor and do it

A person calls this skill once, in a window they opened (*Call a skill*). From then on the
window is a **worker**: it waits for a letter from the supervisor — the session running
`moai-supervise` — does the work the letter hands over by the steps below, reports, and
waits again. Nobody launches this window or drives it: the person opened it, and the
letter is the whole assignment. **The person comes first** — when they speak in this
window, stop waiting and answer them.

## The loop

1. **Say who you are**, once per window

       moai hello --role worker

   The name in the reply is the one the supervisor sends to. moai finds this agent among
   the processes above the command (`claude`, `agy`); a session its hooks already
   registered keeps its name and gains the role. If this window sets `MOAI_AGENT`, that is
   its name — `hello`, the wait and the hooks all go by it. In Codex, moai finds this window
   by the session id Codex sets in its shell (`CODEX_THREAD_ID`) — the row its hooks wrote.
   If `moai hello` says it cannot tell which session this is, Codex did not set it: say hello
   as that row, `moai hello --role worker --as <that name>` (its first context names it,
   `codex-` and eight characters), and pass the same `--as` to every `moai inbox` and
   `moai send` below
2. **Wait for a letter**

       moai inbox --ack --wait 540

   While it waits, `moai agents` shows this window `idle` — that is how the supervisor
   finds it — and once a letter comes, `busy`. If the wait runs out empty, run it again.
   Keep one wait inside your own limit for a shell command, and ask for that limit: Claude
   Code's Bash tool gives a command two minutes unless you pass it a `timeout`, at most ten
   minutes — pass the ten, and 540 seconds fits. Each wait that runs out costs
   one short turn of tokens — the price of a worker that needs no tmux and nobody to wake it.
   With the hooks installed, a letter that comes as a turn ends or a prompt arrives is loaded
   into the conversation and marked read; it is the same letter, so do it
3. **Do what the letter hands over** — read it as "The letter" says, then follow "The
   steps" from where its first line says to start. The steps begin in the root the letter
   names (`Root:`) — if this window stands anywhere else, go there first: `cd` from a
   subdirectory, *Come back to the root* from a worktree
4. **Report it last** (12) and then do what `After the report:` says — wait again (2 here),
   or end the turn

A letter that hands over no work is not work: if it asks something, answer it with
`moai send <from> '<subject>' --reply-to <id> -b -`, and wait again.

## The letter

The first line names the work and where to start — `from step 1` for a new idea,
`from "Carrying on stalled work"` for work a session left behind, `from step 2` for an epic
already unfolded whose first-column members are left. The letter's `from` is the supervisor
you report to, `<supervisor>` below. Every other line fills a slot the steps use; a line the
supervisor adds beyond those — who already said yes to taking over a row that is not yours,
say — belongs to the assignment as well.

- `Model:` — `<model>` and `<difficulty>`.
  `Model:` is a suggestion picked by difficulty before anyone read the code. The model is
  changed by the person (*Change the model* — `/model`), never by you — if this window is not on
  that model, ask the person watching the window to match it, and if it reads harder than it
  looked, raise it the same way to the model that pairs with the difficulty you just measured —
  not one step at a time (haiku → sonnet → opus). Handed `low` but it is `high`, the model is `opus`.
  The grade of the epic-end review (7) is measured on this same rubric, member by member.
  The ladder is Claude Code's. In Codex and Antigravity `<model>` is `—`: the person picked
  the model when opening the window, and the difficulty still picks the review grade
- `Work running alongside:` — the worktrees and work beside you, and the files they hold (4-3)
- `Base branch:` — `<base branch>`. The supervisor read it in the root; do not read it again —
  read inside a worktree, it gives that worktree's own branch
- `Milestone:` — `<milestone>`, the release the epic hangs (1)
- `Root:` — `<root>`, the root checkout's place (4-1)
- `Subdir:` — `<subdir>`, only for a subdirectory project in a monorepo (3). Without it the
  root is the top of the repository
- `After the report:` — `wait again` or `end the turn` (12)
- `Person:` — `here`, or `away` (below)

## When the person is away

When the letter says `Person: away`, decide by recommendation. A design question the notes
do not settle is not asked (4) — settle it the way you would have recommended, and write it
on the issue where the next person reads it

    moai note <id> 'Decided alone: <what you chose> — <why>, and what the other way was'

Nothing else waits on the person either. A model the window is not on is not asked for —
work on the window's model and say so in the reason of 9-1. A row that is someone else's or
nobody's (rule 5) is taken over only on a yes the letter carries; without one, leave that row
and name it in the report.

**Stop at what cannot be undone** — deleting what is not yours, rewriting history someone
else has, a release, anything outside this repository — and report that instead of doing
it. When the person is back in the window, what they say overrides what you decided alone.

## Words per agent

moai plants the same skills for Claude Code, Codex and Antigravity, so the steps in them
are named by what they do — a step written in *italics* is a row of this table. Each agent
types a step its own way — read your own column.

| Step | Claude Code | Codex | Antigravity |
|---|---|---|---|
| Enter the worktree | `EnterWorktree(path)` from the root | `cd` into it and run every command there | `cd` into it and run every command there |
| Come back to the root | `ExitWorktree(keep)` | `cd` to the root and run every command there | `cd` to the root and run every command there |
| Ask the person watching | `AskUserQuestion` | `request_user_input` | ask in the conversation and wait |
| Review the work | `/code-review` | the review this session has, else read the diff yourself | the review this session has, else read the diff yourself |
| Change the model (the person does it) | `/model` | `/model` | — |
| Clear the window (the person, or a supervisor on tmux) | `/clear` | `/new` | `/clear` |
| Call a skill (the person does it) | `/<skill>` | `$<skill>` | ask for the skill by name |
| Wake a session that sits idle (a bonus) | `moai send --wake`, or `SendMessage` when it says so | `moai send --wake` | `moai send --wake` |
| Stop what a review left running | `TaskStop` | — | — |

A `—` is a step that agent does not have, or one moai does not know yet: tell the
person watching and go on without it.

## Before the steps

Before you commit or merge in the root, **only check** that the root still stands on that
branch — run `git symbolic-ref -q HEAD` **in the root**. If it is not `refs/heads/<base branch>`
(detached, or someone switched the branch), do not run that commit or merge: tell the supervisor and
stop. A merge that lands on the wrong HEAD leaves no reference at all once `branch -d` runs.

**Ask it where you already are.** Before the first tracker commit you are still in the root, so
it is one command. From inside the worktree, do not ask with `git -C <root> …` — that shape is
refused there in Claude Code (the git shapes below): *Come back to the root*, ask, and if
work is left in that worktree go back in (*Enter the worktree*)

**Git in a Claude Code worktree session: one plain command per call.** The harness reads each Bash call
and refuses what it cannot prove stays inside your worktree, so the shape matters more than the
intent. The counts below were measured over one repository's transcripts on 2026-09-29 — 714
refusals in all.
- One command per call. `git add X && git commit …` is refused whole — 608 of those 714 were
  compound commands (`&&`, `;`, `||`)
- `-m "…"` on one plain command is fine; a **heredoc** message is the refusal shape (205 cases).
  When the message runs past one line, write it with Write and use `git commit -F <that file>`
- Several git steps in a row: put them in a script file and call it as a bare
  `bash /abs/path/script.sh` with literal arguments and nothing appended. The only script calls
  refused had `&&`, a pipe or `$PWD` after them
- Never build a command or a path with a variable or `$(…)` — that is the second refusal wording,
  `computed at runtime` (119 cases)
- **Do not aim git at the root from inside the worktree.** `git -C <root> status`, `commit` and
  `symbolic-ref` are refused even as single plain commands (28 cases). Root work happens after
  ExitWorktree(keep), and the tracker needs no `-C` at all — `moai` moves that by itself
- Before a tracker commit in the root, look at `git status -- .moai/` first. The path keeps the
  commit from sealing someone's open merge, but it cannot keep it from carrying rows another
  session has not committed yet

## The steps

1. Unfold it in the root — the one way to turn an idea into work is
   `moai idea promote <id> --from -`. Unfold into an epic plus issues even for a single
   issue. Look at `--dry-run` first — that is for this window to see, not to show a person
   and ask. Showing a split plan to a person once is a step of work a person asked for
   directly; what a supervisor hands you is work a person already passed on. Design
   decisions are asked in 4.
   If that idea is already done (someone unfolded it), do not unfold: tell the supervisor —
   unfolding again puts up two epics. **Write a short new title** — a line in the plan
   becomes the issue title verbatim, so copying over an idea title that grew long while it
   was parked spreads that length into the issues. The original text stays on that idea and
   the history leads back to it.
   Then hang the milestone on the epic you unfolded — `promote` brings over the body and the
   release the idea stood in, and a milestone is inherited, so the epic alone carries it to
   every member and to the members added later in 4-3 and 7-1. Hanging the same one again
   changes nothing. **Hang only the `<milestone>` in the letter, and nothing else**: work is
   never pulled into a running release, so a release you noticed running is not yours to
   attach — not to this epic, not to a member you create later. Inside this epic the release
   is inherited, which is the one door that stays open. If `<milestone>` is `none`, this work
   stands outside every release — that is nothing running, or an idea that stood under none,
   or one whose release is already dead, and you cannot tell which from the word alone. What
   came over is still the release that idea stood in, so read the line `promote` printed and clear a
   release that has already shipped or been deferred with `moai edit <epic> --milestone none`;
   a dead one is named on stderr. Under a deferred one the whole plan is out of the plan:
   not in `ready`, not in `held`, no warning
     moai edit <epic> --milestone <milestone>
2. Pick the members up with `moai mv <member> in_progress --from todo` and commit in the root.
   **Pass the column you saw** — this is a place where several sessions share one `.moai`,
   and overwriting a row picked up beside you means two of you do the same work. A non-zero
   code means it is taken, so leave that member and tell the supervisor. The root is shared
   by every session — a commit made while someone has a merge open (MERGE_HEAD) seals that
   merge with its own subject. So give the tracker commit a path. With a merge open git
   refuses it, so wait for that merge to finish and run it again
     git commit -m "chore(tracker): pick <epic> up in a worktree" -- .moai/
   **A member that is someone else's, or nobody's, is asked about** — the hook refuses that
   pick-up (rule 5). Ask the person watching this window — a yes the letter already carries
   counts; on a yes, run the line the refusal hands you (`--take -m '<who said yes>'`), on a
   no leave that member and tell the supervisor
3. Right after the commit in 2, branch from the local <base branch> with
   `git worktree add -b worktree-<epic> .worktrees/<epic> <base branch>` and go in
   (*Enter the worktree*). The name is the unfolded epic's id, not the idea's. Until the
   worktree stands, the other sessions in the root read this member as their own focus.
   **If the root is not the top of the repository** (a subdirectory project in a monorepo) the
   worktree stands for the whole repository, so once inside, move to the same subdirectory in
   it and work there — standing at the worktree top, `moai` walks up and finds the root's
   `.moai` to write, and the hook does not count edits under `.worktrees/`. `<subdir>` is that
   relative path, filled in by the supervisor; if the letter carries no `Subdir:`, the root
   **is** the top and this step does not exist
     cd <subdir>
4. Do not guess a design decision that is not in the notes — ask (*Ask the person watching*);
   a person is watching the worker's window. With `Person: away` in the letter, decide by
   recommendation instead ("When the person is away")
4-1. **The tracker you edit is always the root's.** `<root>` is the root checkout's place,
   filled in by the supervisor — do not guess it from inside the worktree. **The tool moves
   that by itself** — even a bare `moai` typed inside the worktree reads and writes the
   root's tracker, and when it writes, one line says where. `moai -C <root> <command>` lands
   in the same place, so writing it that way is fine too. Short of a branch with no tracker
   in the root, the only thing that stops the move is `MOAI_HERE`, so **do not turn it on** —
   turn it on and that worktree's `.moai` changes, and the snapshots conflict on the merge
   (and merging them overwrites someone else's rows).
   Put `-e <epic>` on an idea you park mid-epic — it does not keep the epic open, and 7-1
   reclaims it through that even if the window is cleared or the work is taken over.
   **Give a review subagent the same words.** If that worktree's `.moai` changed anyway,
   undo it with `git checkout -- .moai`, and if the row was already committed, undo that
   commit too and park it again from the root
4-2. **If you test tmux, do it on a separate server only** — `env -u TMUX tmux -L <unique name>`
   on every call. Put the epic id in the name so it cannot collide with the test servers of
   the workers beside you or of a review subagent. `-S <socket>` works too, but a socket path
   does not stand past the unix limit (about 100 bytes), and a scratchpad path is usually too
   long. Never use `kill-server` or `kill-session` without `-L`/`-S`: inside tmux a bare
   `tmux` goes to the person's default server and kills every session, and `TMUX_TMPDIR` does
   not fence it in. A script that calls `tmux` inside itself cannot be given `-L` by hand, so
   run it with a wrapper at the front of `PATH` that calls the real `tmux` by absolute path
   and inserts `-L`. Do not send keys into a pane someone else raised. If you raise a test
   agent on that server, keep its cwd outside the root (the scratchpad) — raised in the root,
   it registers in this repository's `moai agents` beside the real workers.
   **Give a review subagent these words too** — it was a review subagent that killed a whole server
4-3. **If you would have to touch a file that work running alongside holds, do not fix it** —
   the files named by `Work running alongside` in the letter, or files a sibling branch in
   `git worktree list` already changed
   (`git diff --name-only <base branch>...<sibling branch>`). When two of them change the
   same place, one waits for the other at the merge. If this epic cannot deliver what it
   promised without that, it is not an idea but a member — create it with
   `moai -C <root> add '<what>' -e <epic>`, leave it in the first column, and name it in 12
   as **a member left because the work beside it holds the file**, together with that other
   work. The supervisor sends it once that work is done. Do not defer it
5. **Do not review member by member.** When one member is finished, run the tests, commit and
   move to the next — the review looks at the whole epic once, in 7, after every member is
   finished. One review is expensive; do not call it as many times as there are members. The
   cost of member 2 piling onto a bug in member 1 is paid in that one review.
   `low`·`medium`·`high` is the rubric the model in the letter was picked on, and the same rubric measures
   the members when you pick the grade in 7.
   `low` — text, comments, a one-line fix; behaviour unchanged
   `medium` — a behaviour change inside one file, ringed by tests
   `high` — several files, the write path, concurrency, the storage format, hooks; hard to undo
6. When the members' work is all done, pull <base branch> into the worktree, resolve the
   conflicts and run the tests. Fix things here — while the worktree stands, rule 2 blocks
   edits in the root
7. Before merging, review the whole epic (*Review the work*) — the members were not reviewed
   separately, so this once is the only review. In Claude Code that is
   `/code-review <grade> --fix`. In Codex and Antigravity it is the review that session has,
   and where it has none, read the whole diff yourself with the angle and the five places
   below, and fix what you take in. **It runs inside this session** — never start another
   agent program for it: `codex review`, `codex exec` and `agy -p` each start one.
   **The grade is one step above the heaviest member's difficulty** — `medium` if the members
   are all `low`, `high` if one is `medium`, `xhigh` if one is `high`.
   **If any member touched the write path, concurrency, the storage format or hooks**, it is `max`.
   Raise it one more step if the epic crosses surfaces or carries several design decisions. If you hesitate, raise it.
   **A worktree that carries members of two epics is measured as one epic and then raised one more step** — the review has to read both epics' contracts at once.
   `max` is the top of the ladder: a step above it is still `max`.
   The model follows that grade — `medium` means `sonnet`, `high` and up means `opus`.
   If the window is not on that model, ask the person watching it to change it
   (*Change the model*) before you call — as in `/model opus` in Claude Code (a review agent
   inherits the window's model). Those models are Claude Code's: in Codex and Antigravity the
   grade still stands, and the window keeps the model its person picked.
   Write the grade you picked and why in one line in the angle (`-b`). The diff runs from
   where the branch left <base branch> (`git merge-base <base branch> HEAD`). You pulled
   <base branch> in 6, so the conflict resolution is inside it too. Create the review issue
   (rule 3)
     moai add 'review — <what you are looking at>' -t review --parent <epic> -b '<what you are looking for and why>'
   This line is called from the worktree too, so run it as `moai -C <root>`, per 4-1.
   **In the same breath, stand the finished members in `review`** — while the review runs
   nobody is working on them, and `in_progress` on the board says somebody is. One id per
   call, as in 2
     moai -C <root> mv <member> review --from in_progress
   `--from in_progress` passes over the members left in the first column by 4-3 (and by 7-1
   when you came back from 8) — this review does not see them, so they do not stand in it.
   moai answers that such a member already stands in the first column and moves nothing, with
   a non-zero code: that is the pass-over, not a lost pick-up as in 2. moai answers in the
   screen's language (`MOAI_LANG`, or `lang` in the user config), so read what it says, not
   the words: if it refuses `review` itself and lists the columns there are without it — in
   English "`review` is not a column" — this repository's columns have no `review` and the step
   does not exist here: leave the members where they stand and go on. If it answers that the
   member already stands `review`, you came back from 8 — leave it. What you take in goes in
   a separate fix: commit; what you hand on goes in a note with the issue id.
   In Claude Code only: when the worktree's hook cannot see a review issue created or picked
   up in the root and blocks you — a binary from before the hook moved the tracker to the root
   reads that worktree's snapshot only — run a review subagent with the same angle, grade and
   `--fix` scope. A subagent inherits the window's model, so pass the model for the grade
   above in `Agent`'s `model`. Keep the review issue, the angle (`-b`), the text note and the
   closing `-m` as they are. Any other refusal, such as a missing angle, is not worked around:
   fix it the way the refusal's own command says
   **Five places the review keeps finding.** They do not stand in for the angle — what this
   epic actually did is the angle, and these go on top of it
   1. A struct or function inserted above another takes over the doc block of the item below
      it, and that comment now sits on code it does not describe
   2. Does a test actually go red on a revert — is what it measures in one place. A count
      held per row whose inside is a `OnceCell` is 0 or 1 whatever happens, so the timing it
      was meant to pin went back whole with nothing red
   3. Is there only one place that sets it up — a value put in place once at start-up is put
      back to its default by every other path that builds the same thing again
   4. Do the comments and the docs say what the code actually does
   5. Does anything newly open on a path that never opened it — not "is a lock held while
      opening", which is half of it. A read path that never opened the config and now parses
      it stops on a config that is a FIFO, lock or no lock
   **They have to reach the review itself, not only `-b`.** The angle on the issue is what the
   next person reads; the review command does not read the issue. Going through a subagent,
   put these five in its prompt; going through the command, hold them against what came back
   before you take the findings in
   **While the review is running, do not touch this worktree's branch or its working tree.**
   A review that fixes leaves its fixes in the working tree uncommitted, so `reset --hard`,
   `rebase` and `commit --amend` throw them away — that has happened, told to do it by a
   supervisor saying "it is before the merge, so it can still be fixed". Nothing blocks it;
   this line is what holds. Fixing a commit subject waits until the review has returned.
   **When it returns, stop what it left running before you touch the tree**
   (*Stop what a review left running* — `TaskStop` in Claude Code) and read the working
   tree's status. A sweep subagent still alive writes its own version into this same worktree
   and covers a commit you already made without a word, and a `cargo test` after that
   measures that agent's files rather than yours — that has happened too, and it also burned
   an hour and a half in a worktree that was gone.
7-1. Before merging, go back over the ideas parked mid-epic
   (`moai -C <root> show --type idea -e <epic>` and what this window remembers) and what the
   review handed on — **can the epic deliver what it promised without them.** If not, it is
   not an idea but an unfinished member. What you sorted as "not for now" while parking has
   these mixed in — the one waiting on a person's decision, the one pushed out because a
   worker beside you held that file. This step sits after 7 so that it sees what 7's review
   handed on too. Unfold such an idea as a member of the epic already standing — write only
   `- issue` lines in the plan; the idea closes by itself and its source stays. You type this
   from the worktree, so pin the root into the line (4-1). **If that idea is already done, do
   not unfold it** — someone unfolded it, or you came back from 8 and are going round again.
   promote unfolds a closed idea too, and the same member stands twice
     moai -C <root> idea promote <idea id> -e <epic> --from -
   Do not do a reclaimed member here: merge with it left in the first column — work that has
   not been through 7's review does not get mixed into the merge, and a member still standing
   keeps the epic open. Do not `defer` that member. Deferring it closes the epic without its
   promise delivered. 7's review did not see that member, so write it in the `Next:` note in
   11 — the window that closes the epic with that member calls the epic-end review again
7-2. **If the epic ran inside a milestone, sort what you handed on once more, by the release
   bar.** A review makes its findings without regard to the release bar, so fixing all of them
   inside pushes the release out by as many findings as there are, and sending all of them
   outside ships with bugs in. This window is what sorts them — you already decided in 7 what
   to take in and what to hand on, so this is the extension of that. **Bug-level stays
   inside**: create it as a member of that epic (`-e <epic>`) or under an epic in the same
   milestone. **What this release can do without goes outside** — not "not doing it", but
   "not in this release"
     moai -C <root> edit <that row> -e none --milestone none
   **Pass `-e none` with it.** A milestone is inherited from the epic, so on an epic member
   `--milestone none` alone changes nothing and comes back with one line saying the place
   comes from the epic and cannot be cut — a row 7-1 reclaimed stands as that epic's member,
   so take it out of the epic here as well. That the row stops keeping the epic open is the
   point: it is a row decided out of this release.
   **Bug-level is measured with the words that already exist** — does a `#bug` tag fit, and
   can a `Regression-of:` line be written (did something already merged break). Those two are
   inside; the rest is outside. A new axis is not made because it would become a fourth
   vocabulary beside the column, the kind and the defer
7-3. **If the repository keeps a CHANGELOG, check that this epic's line stands in the section
   for the release being prepared**, and write it if it does not. Write it **here, in the
   worktree, before the merge**: after 8 the worktree is gone and the only checkout left is
   the root, which every session shares and where the only commits that belong are the
   tracker's and the merge itself. Committed here it rides the merge commit instead
     git commit -m "docs(changelog): <what this epic changed> (<epic>)" -- CHANGELOG.md
   This window is the only one that knows what the epic did, and it is the only one that
   knows what was taken out as well as what went in — a section filled in later from commit
   subjects shows what was added and misses what was removed, because a removal stands under
   a revert subject of its own. It is cheaper here than in the window that closes the
   section: v0.1.1 stood with 327 commits behind it, four of which touched the CHANGELOG, and
   three epics named in its section out of the twenty-four the release held; the rest were
   written by the window that closed it, 91 lines in one go. The release workflow cuts that
   section by version name and hands it to `--notes-file` as it is, so a missing section
   reads to whoever receives it as the whole release. **Nothing checks this** — a check here
   would be a gate, and an empty section must not stop a release
7-4. **If the repository keeps a wiki** (`moai wiki ls` lists pages), ask once whether this
   epic changed what a person does — a key, a command, a flag, a file, a format, a procedure.
   If it did, follow the `moai-wiki` skill and commit what it wrote for the same reason as
   7-3 — here, in the worktree, before the merge. `<wiki dir>` is `dir` in `moai wiki ls --json`
     git add -- <wiki dir>
     git commit -m "docs(wiki): <what changed> (<epic>)" -- <wiki dir>
   If it did not, write nothing. **Nothing checks this**
8. *Come back to the root* — remove the worktree from inside it and this window stands in a
   directory that is gone.
   Before merging, check that the root stands on <base branch> — if it does not, do not merge:
   tell the supervisor
     git symbolic-ref -q HEAD                  it has to be refs/heads/<base branch>
   Merge in the root, **in one call**. Overlap with the workers beside you was split when the
   supervisor sent the work, and where it still collides, undo as below and resolve in the
   worktree — do not go looking for the other session to tell it. Do not use `--no-commit`.
   Without `--no-ff` it ends as a fast-forward and no merge commit stands
     git merge --no-ff worktree-<epic> -m "merge: …"
   If the root's `.moai` holds uncommitted rows from another session the merge is refused —
   take them in first with a commit with a path, as in 2. If it stops on a conflict, do not
   resolve it in the root — undo with `git merge --abort`, go back into the worktree
   (*Enter the worktree*) and run again from 6
9. Once the merge has really landed, remove the worktree and the branch from the root with
   `git worktree remove .worktrees/<epic>` and `git branch -d worktree-<epic>`
9-1. Before closing, leave one line per member **on what did this work** in this window —
   leaving out the members left in the first column by 7-1 and 4-3, which nobody did. Not the
   suggestion in the letter but the model that **actually ran** in this window. The line below
   was filled in by the supervisor as a suggestion, so if you raised it, or the window was on
   a different model from the start, correct the model and the difficulty to the real ones and
   write why in the reason — the next person reads "what was put on work of this size" there.
   It is a note, not a field: the journal is not read to compute state and derived values are
   not stored. The supervisor does not fill `<vendor>` or `<count>` — the vendor is
   `anthropic`, `openai` or `google`, and the model is its real name (`opus-5`), not the
   `/model` alias — the `<model>` the supervisor filled in is an alias (`opus`) or `—`, so
   write the real name even if you did not change models.
   `<count>` is the tokens this window used. **If you do not know the token count, drop
   `tokens=<count>` whole** — do not write 0 and do not estimate. **One line per id** —
   write the same line on several ids and the tokens multiply by the number of ids. A window's
   tokens cannot be split per member, so write them on **one member only** and leave
   `tokens=<count>` out of the other members' lines.
   Quote free text with single quotes — inside double quotes the shell expands backticks and
   `$(…)` as commands. If the text itself contains a single quote, stream it from stdin with `-b -`
     moai note <member> 'model: <vendor>/<model> tokens=<count> (<difficulty> — <why>)'
10. Close them after that. **Run `moai mv <member> done` only once that merge has really
   landed** — a worker moved them before the merge and had to undo it. It closes a member
   from `review`, where 7 stood it, and from `in_progress` where there is no `review` column
   alike. Do not close the
   members left in the first column by 7-1 and 4-3 — those members keep the epic open. While
   the worktree still stands, the hook reads this work as a sibling worktree's and cannot
   refuse a review closed without `-m`. Close the review issue leaving what came out of it
     moai note <review id> -b - < <review text>   the reviewer's own words (summarize past 64KB)
     moai mv <review id> done -m '<what you took in, what you handed on>'
   If the text runs past 64KB, summarize it — put `Summary: original <size>KB agent-<task-id>` on
   the first line, keep every finding's number and place, and shorten only the sentences. Leave fences and indentation alone
   Leave the tests passing in the root with a commit with a path, as in 2
11. Leave the line to take over from — `moai note <epic> 'Next: …'` — and take it into the root
   with a commit with a path as in 2. It is written after the commit in 10, so leaving it out
   leaves it in the shared root where someone else's commit sweeps it up. If anything is left
   (a background review, say), finish it before the note — the supervisor reads the note as
   this work being over; what you cannot finish, name in the report (12)
12. Report with a letter to the supervisor, **last of all**. It carries the merge hash,
   the unfolded epic's id, a line or two of summary, what you handed on and any new ideas,
   the members reclaimed in 7-1 and left in the first column,
   the members left in 4-3 because the work beside you held the file, with that other work
   named, and the wiki pages 7-4 changed — or that it changed none — and anything still
   running that 11 could not finish: the supervisor does not clear a window whose report
   says so. Write it to a file outside the repository (your scratchpad, or a temporary
   file) — nothing is held by now, so a file in the shared root is refused or left behind —
   and send it
     moai send <supervisor> 'report: <epic>' --reply-to <letter id> -b - < <report file>
   Then **say when the window can be cleared**, in one line to the person watching. The
   context lives in the tracker, not in the conversation: issue bodies, notes, review texts,
   commit messages. If you can see your own context usage, put that number in the line too.
   **Say the opposite in the same line** — not to clear while a review is running in the
   background, while a merge conflict is being resolved, while waiting on a person's answer,
   or after the supervisor's next message has arrived in this window. Clearing then loses what
   is not yet moved into the tracker, or the message that arrived.
   Then do what `After the report:` says. With `wait again`, go back to waiting (the loop's 2)
   — a person who wants the window cleared stops the wait, clears it (*Clear the window*) and
   calls this skill again (*Call a skill*). With `end the turn`, end it — on tmux
   the supervisor may check the report and clear this window itself once it sees the `Next:`
   note stand, and then wakes it with the next letter

## Carrying on stalled work

A session died holding a member of `<epic>`, the epic the letter's first line names; that
member still stands picked up. Read how far it got (`moai show <epic>`, its history and
notes), then

- If the worktree is there, go in (*Enter the worktree*), read how far it got with
  `git log <base branch>..HEAD` and `git status`, and carry on. One raised before the
  shared place stands at `.claude/worktrees/<epic>` — remove it from there in 9
- If it is not, raise it again from the root. If the branch survives, on that branch
  (`git worktree add .worktrees/<epic> worktree-<epic>`); if it does not,
  `git worktree add -b worktree-<epic> .worktrees/<epic> <base branch>`
- **If the root is not the top of the repository** (a subdirectory project in a
  monorepo), go into the worktree and then move to the same subdirectory inside it and
  work there — standing at the top, `moai` finds and writes the root's `.moai`, and the
  hook does not count edits under `.worktrees/`. `<subdir>` is that relative path, filled in
  by the supervisor; with no `Subdir:` in the letter, the root is the top and this step
  does not exist
    cd <subdir>
- The member's column is already picked up — do not pick it up again. **If its assignee
  is not you** (`moai show <member>`), ask the person watching before you carry it on; on
  a yes, `moai mv <member> <its column> --from <its column> --take -m '<who said yes>'` — the
  column stays, the assignee becomes you, and a note keeps whose it was
- The note in 9-1 records this window's share only. Append `reclaimed work, the previous
  session's share is unknown` to the end of the reason — the previous session's model and
  tokens are written nowhere, and without it the whole member reads as this window's work
- Then go on with the steps from 4-1 to the end
