## The milestone branch

When `Base branch:` reads `milestone/<milestone id>`, the work is inside a milestone —
`<milestone id>` below is the part after `milestone/`. Every epic of that milestone merges
into that branch, not into `<root branch>`; `<root branch>` takes the milestone branch in
once, at the release, and the release is the person's, not yours. The branch is checked out
in a long-lived worktree of its own, `.worktrees/milestone-<milestone id>`, because the root
stays on `<root branch>` and git checks a branch out in one place only. You do not work in
it — you merge there (8) and nothing else — and you never remove it: it goes after the
release. Outside a milestone this section does not exist. `Milestone:` can say `none` while
`Base branch:` names a milestone — that milestone stands but has not started running, and
`promote` still carries it over in 1; the branch is where this work goes all the same, and
in 1 you hang `<milestone id>`, not `none`.

**Raise it if it is missing**, from the root, right before the `worktree add` of 3 — look
at `git worktree list` first. The first line raises the branch from the local
`<root branch>`; the second is for a branch that stands while its worktree does not. If
`worktree add` says the branch or the place already exists, a worker beside you raised it
first — look at `git worktree list` again and carry on with what stands

    git worktree add -b milestone/<milestone id> .worktrees/milestone-<milestone id> <root branch>
    git worktree add .worktrees/milestone-<milestone id> milestone/<milestone id>
If the repository's own worktree convention adds something to a new worktree (a link for
the build output, say), add it to this one too.

**Git aimed at the milestone's worktree runs from the root**, where you stand —
`git -C .worktrees/milestone-<milestone id> …`. That worktree is shared by every worker of the
milestone, like the root: if git refuses a merge there because one is already open
(`MERGE_HEAD`), it is another worker's — never `merge --abort` it; wait for it to finish
and run yours again. Before you merge into it, check its HEAD the way
the root's is checked; if it does not stand on the milestone branch, do not merge: tell the
supervisor

    git -C .worktrees/milestone-<milestone id> symbolic-ref -q HEAD      it has to be refs/heads/milestone/<milestone id>
**The milestone branch takes `<root branch>` in only at the release.** If this epic needs a
fix that landed on `<root branch>` after the milestone branch split off, take it in at that
moment, from the root, and then pull the milestone branch in 6 as usual. If it stops on a
conflict, resolve it inside the milestone's worktree, by its absolute path ("Stay in the
root"), never in the root

    git -C .worktrees/milestone-<milestone id> merge --no-ff <root branch> -m "merge: take <root branch> in for <epic>"


## Step 1 — unfold a backlog

1. Unfold it in the root — the one way to turn a backlog item into work is
   `moai backlog promote <id> --from -`. Unfold into an epic plus issues even for a single
   issue. Look at `--dry-run` first — that is for this window to see, not to show a person
   and ask. Showing a split plan to a person once is a step of work a person asked for
   directly; what a supervisor hands you is work a person already passed on. Design
   decisions are asked in 4.
   **In a subagent, measure before you unfold** — read the backlog against the rubric in 5
   first, and if it reads heavier than `Model:` says, hand back `Model: raise to complex — <why>` and end.
   Nothing is unfolded yet, so the worker raised after you starts from step 1 as you did.
   If that backlog is already done (someone unfolded it), do not unfold: tell the supervisor —
   unfolding again puts up two epics. **Write a short new title** — a line in the plan
   becomes the issue title verbatim, so copying over a backlog item title that grew long while it
   was parked spreads that length into the issues. The original text stays on that backlog and
   the history leads back to it.
   Then hang the milestone on the epic you unfolded — `promote` brings over the body and the
   release the backlog stood in, and a milestone is inherited, so the epic alone carries it to
   every member and to the members added later in 4-3 and 7-1. Hanging the same one again
   changes nothing. **Hang only the `<milestone>` in the message, and nothing else**: work is
   never pulled into a running release, so a release you noticed running is not yours to
   attach — not to this epic, not to a member you create later. Inside this epic the release
   is inherited, which is the one door that stays open. **If `<milestone>` is `none` but
   `Base branch:` names `milestone/<milestone id>`**, that milestone stands and has not started
   running — hang that `<milestone id>` in its place: `none` would clear the release `promote`
   carried while the merge still lands on its branch. Otherwise, if `<milestone>` is `none`, this work
   stands outside every release — that is nothing running, or a backlog item that stood under none,
   or one whose release is already dead, and you cannot tell which from the word alone. What
   came over is still the release that backlog stood in, so read the line `promote` printed and clear a
   release that has already shipped or been deferred with `moai edit <epic> --milestone none`;
   a dead one is named on stderr. Under a deferred one the whole plan is out of the plan:
   not in `ready`, not in `held`, no warning

       moai edit <epic> --milestone <milestone>
   Then hang `<due>` on the epic the same way, unless it is `none` — `none` hangs nothing
   and does not clear a deadline `promote` carried over from the backlog. The members are
   bounded by it without carrying it. If the write is refused because the day is later than
   the milestone's deadline, leave the epic's deadline as it stands and name it in the report

       moai edit <epic> --due <due>
