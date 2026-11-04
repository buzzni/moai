7-1. Before merging, go back over the backlog parked mid-epic
   (`moai -C "<root>" show --type backlog -e <epic>` and what this window remembers) and what the
   review handed on — **can the epic deliver what it promised without them.** If not, it is
   not a backlog item but an unfinished member. What you sorted as "not for now" while parking has
   these mixed in — the one waiting on a person's decision, the one pushed out because a
   worker beside you held that file. This step sits after 7 so that it sees what 7's review
   handed on too. Unfold such a backlog item as a member of the epic already standing — write only
   `- issue` lines in the plan; the backlog closes by itself and its source stays. You type this
   from the worktree, so pin the root into the line (4-1). **If that backlog is already done, do
   not unfold it** — someone unfolded it, or you came back from 8 and are going round again.
   promote unfolds a closed backlog too, and the same member stands twice

    moai -C "<root>" backlog promote <backlog id> -e <epic> --from -
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

    moai -C "<root>" edit <that row> -e none --milestone none
   **Pass `-e none` with it.** A milestone is inherited from the epic, so on an epic member
   `--milestone none` alone changes nothing and comes back with one line saying the place
   comes from the epic and cannot be cut — a row 7-1 reclaimed stands as that epic's member,
   so take it out of the epic here as well. That the row stops keeping the epic open is the
   point: it is a row decided out of this release.
   **Bug-level is measured with the words that already exist** — does a `#bug` tag fit, and
   can a `Regression-of:` line be written (did something already merged break). Those two are
   inside; the rest is outside. Do not make a new tag or field for it

7-3. **If the repository keeps a CHANGELOG, check that this epic's line stands in the section
   for the release being prepared**, and write it if it does not. Write it **here, in the
   worktree, before the merge**: after 8 the worktree is gone and the only checkout left is
   the root, which every session shares and where the only commits that belong are the
   tracker's and the merge itself. Committed here it rides the merge commit instead. Aim the
   line below at the worktree like every step there (`git -C "<root>/.worktrees/<epic>" commit …`,
   "Stay in the root") — and 7-4's two lines the same way.
   **One commit for the epic**, written once the members and the review's fixes are in —
   every line the epic needs goes into it. If a fix lands after it and changes what the epic
   ships, put the CHANGELOG line in that fix's own commit rather than raising a second
   `docs(changelog):`

    git commit -m "docs(changelog): <what this epic changed> (<epic>)" -- CHANGELOG.md
   This window is the only one that knows what the epic did, and it is the only one that
   knows what was taken out as well as what went in — a section filled in later from commit
   subjects shows what was added and misses what was removed, because a removal stands under
   a revert subject of its own. The release notes are that section as it stands, so a
   missing line reads to whoever receives them as a change that never shipped.
   **Nothing checks this** — a check here would be a gate, and an empty section must not
   stop a release

7-4. **If the repository keeps a wiki** (`moai wiki ls` lists pages), ask once whether this
   epic changed what a person does — a key, a command, a flag, a file, a format, a procedure.
   If it did, follow the `moai-wiki` skill, its `moai wiki` calls aimed at the worktree (`moai -C "<root>/.worktrees/<epic>" wiki ls` —
   bare, they read the root's pages), and commit what it wrote for the same reason as 7-3 — here, in the worktree, before the merge. `<wiki dir>` is `dir` in `moai wiki ls --json`

    git add -- <wiki dir>
    git commit -m "docs(wiki): <what changed> (<epic>)" -- <wiki dir>
   If it did not, write nothing. **Nothing checks this**
   Also ask whether a finding of this epic's review is one a single rule would have stopped — if so, follow Lessons in `moai-wiki`, in the same commit

