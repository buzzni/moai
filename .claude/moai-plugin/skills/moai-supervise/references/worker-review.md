7. Before merging, review the whole epic with `/code-review <grade> --fix <root>/.worktrees/<epic>`
   — aimed at the worktree: you stand in the root, and a call with no target reviews the
   root's diff. The members were not reviewed separately, so this once is the only review.
   **It runs inside this session** — never start another agent program for it.
   **The epic-end review runs on the `review` model at `review_grade`** — read both and `review_raise` from
   `moai models --json` (defaults `opus`, `xhigh`, `never`).
   With `review_raise = "write-path"`, raise one step if any member touched the write path, concurrency, the storage format or hooks, or if the worktree carries members of two epics. Raise at most once, capped at `max`; `never` leaves the configured grade unchanged.
   A worktree that carries members of two epics is measured as one epic. If a worker's proposed grade differs, the supervisor corrects it from the settings and records why in the review issue angle.
   If the window is not on that model, ask the person watching it to change it before you
   call — `/model <that model>` (a review agent inherits the window's model).
   **In a subagent** the review is a subagent of your own — still this session, not another
   agent program — because a review called from you runs on your model. Read the `review`
   model from `moai models --json`, create the review issue and stand the members in `review`
   as below, **pick the review issue up** —
   `moai -C "<root>" mv <review id> in_progress --from todo` — and only then raise it on the
   review brief ("The review brief" in `worker-branches.md`), every slot filled, for the reason below.
   Save it as `<scratchpad>/<epic>/brief.md`, as that section says, and pass the path, not the text.
   It leaves its fixes uncommitted and the reviewer's
   words on the review issue, and hands back what it fixed and what it left — take that in as
   below

       Agent(subagent_type: "general-purpose", model: <review model>, description: "review <epic>", prompt: "Read <scratchpad>/<epic>/brief.md whole before you start, then follow it")
   Write the grade you picked and why in one line in the angle (`-b`). The diff runs from
   where the branch left <base branch> — `git -C "<root>/.worktrees/<epic>" merge-base <base branch> HEAD`;
   asked in the root, `HEAD` is the root's own tip and the range comes out empty. You pulled it
   in 6, so the conflict resolution is inside it too. Read the epic body and put the contents
   of its `Review focus / 리뷰에서 볼 것` section (either heading) first in the review issue angle,
   followed by any focus discovered during development — a section of several lines or one
   holding a single quote goes in through `-b -` from `<scratchpad>/<epic>/angle.md`, not inside `-b '…'`. Create the epic directory first; never share the angle file with another epic.
   If neither heading exists, write the angle as before. Create the review issue (rule 3)

       moai add 'review — <what you are looking at>' -t review --parent <epic> -b '<what you are looking for and why>'
   Run this line as `moai -C "<root>"`, per 4-1, wherever the shell stands.
   **In the same breath, stand the finished members in `review`** — while the review runs
   nobody is working on them, and `in_progress` on the board says somebody is. One id per
   call, as in 2

       moai -C "<root>" mv <member> review --from in_progress
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
   **Then pick the review issue up, and only then call the review** — in a session as in a
   subagent: `moai -C "<root>" mv <review id> in_progress --from todo`. You stand in the root,
   so the hook reads a review issue nobody picked up under `.worktrees/<epic>` as that
   worktree's, and rule 3 refuses the review — asking for another review issue, which it
   reads the same way, while the first one stays open and keeps the epic from closing.
   A refusal from the hook, such as a missing angle, is not worked around: fix it the way the
   refusal's own command says.
   **If the review stays refused** — the review issue picked up, every refusal's command run,
   and rule 3 still ties the review to a row that is not this epic's — do not work around the hook
   and do not start another agent program: review by hand, as
   "When the review stays refused" in `worker-branches.md` says.

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
   next person reads; the review command does not read the issue. Hold these five against what
   came back before you take the findings in

   **While the review is running, do not touch this worktree's branch or its working tree.**
   A review that fixes leaves its fixes in the working tree uncommitted, so `reset --hard`,
   `rebase` and `commit --amend` throw them away — even when it is before the merge and
   looks fixable. Nothing blocks it; this line is what holds. Fixing a commit subject waits
   until the review has returned.
   **Nor is the branch rebased or squashed when it merges** — it goes in with
   `git merge --no-ff` as it stands (8). `moai show` finds an issue's commits by the id in
   their subject, and a squash folds them away; the review's fixes stay as their own `fix:`
   commits; and rebasing a branch that already holds merges — the milestone branch — changes
   the merge hashes the `Report-checked:` notes point at. Rewording a subject on this branch
   before the review starts is fine — nobody else has it yet.
   **When it returns, stop what it left running with `TaskStop` before you touch the tree**
   — in a subagent, the review subagent too if it still runs — and read the working tree's
   status. A sweep subagent still alive writes its own version into this same worktree and covers a commit you already made without a word, and a
   `cargo test` after that measures that agent's files rather than yours.

