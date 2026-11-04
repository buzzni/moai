**0. Reclaim first — work that lost its place.** When a session dies the row it picked
up stays `in_progress` and nobody carries it on. Look at this before picking new backlog.

    moai status --json                     the ids of warnings whose kind is "stranded"
                                           (inside a worktree it stands only with `--worktree`)
    moai show <id>                         the `Place` line — one of the three words below
                                           (inside a worktree this too needs `--worktree`)

`stranded` is a row that was picked up while no live worktree holds that work — either
the worktree is gone, or **the work was being done in the root with no worktree**. This
row alone does not tell the two apart: if `ListAgents` shows a `busy` session of this
repository, it may be that one, so ask what it is holding before handing the work on —
a worker with a message, any other window through the person.

The `Place` line (`place` under `--json`) has three values. **Only `none` is handed on.**

    <path> (<branch>)  at        it runs there. Aim at it and carry on
    not showing yet    fresh     just picked up — the gap while the worker raises its worktree. Leave it
    none               lost      it lost its place — only this one is reclaimed

**A place is read from two things only** — the worktree's name (a directory named `<id>`,
or a branch `<id>` or `worktree-<id>`, the row's epic counting too) and the pick-up mark
`moai mv` leaves in the worktree it was typed in. Sibling snapshots are not read for it, so
a broken one never hides a verdict. **One gap remains:** a row picked up in the root and then
carried into a worktree whose name is not its id (an agent's `worktree-agent-<hash>`) has
no mark there and shows `none` while that session may still be at work — the hook reads
the same two things and has the same gap. A worker that follows its steps raises the
worktree under its epic's name, so the name already points at the row.

**The `Trouble in sibling worktrees <n>` on the person's screen is a different matter.**
That one counts every worktree whose snapshot could not be read, and the other problems
met while overlaying too (a snapshot with unparseable rows, a worktree list that could not
be read). It says there is something to fix and leaves `stranded` as it is. Machines see
the broken worktrees by `broken_worktrees` under `status --json` — look at that key when
you are hunting for the worktree to fix.

A row picked up less than an hour ago does not show (that is the gap while a worker
raises its worktree). **A worktree that is still there while the session working in it
died does not show under `stranded`** as long as its name or mark points at the row (the
gap above aside) — it is a worktree in `git worktree list` whose
worker — the session you sent that work to — no longer answers. **A name gone from
`ListAgents` is not an ended session**: the name belongs to the process, so a window resumed
with `claude --resume` comes back under a new name, still in that worktree. A session that
still stands there, idle, has not ended either: its person may be answering it, or it may be
holding your message for approval. Look for the worker by its worktree, not its name — ask
the person which window works in it. Hand its work on
only once the person says that window has ended; until then it is that worker's. When it
comes back under a new name, move what you keep under the old one — the work you sent, a
refusal — to the new name (2).

- When there is such work, hand carrying it on to one idle worker **before any new
  backlog**. Send the message in 3 with its first two lines changed to the two below, and
  the rest filled as 3 says (`<other work>` too — 4-3 points at that line). The steps file
  sends the worker on to the section the first line names, which stands beside it in
  `references/worker-branches.md`

      You are handed the stalled work in <epic> — the previous session did not finish it. Read <steps file> and follow its steps from "Carrying on stalled work".
      Read first: moai show <epic> (history and notes) · moai show <member> (the place too — a place stands on work only)

- **Whether it is carried on or put down is not the supervisor's call.** If it looks like
  work to put down (`moai mv <id> todo`, `moai defer <id> -m '<why>'`), ask the person
- Work handed on to be carried is, like a backlog item, not sent again until its report is checked

