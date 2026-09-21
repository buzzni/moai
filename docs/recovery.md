# Recovery

What to do when the tracker files, a merge, or a release ends up in a state you
did not intend. Everything here is a file in your repository, so almost every
answer is "look at it, then commit the version you want".

## The files

```
.moai/issues.jsonl              one line per issue, sorted by id. The snapshot is the truth.
.moai/journal/<email>.jsonl     append-only history, one file per writer. Never read to compute state.
.moai/journal.jsonl             the old single file. Still read; nothing is written there any more.
```

`issues.jsonl` is what every command reads. The journal records `create`,
`status`, `note` and `rm`; field edits are not recorded. If the journal is lost,
you lose history, not state.

**Several journal files is the normal shape.** The name comes from the writer's
email with `@` and `.` folded to `_` (`raven@buzzni.com` →
`raven_buzzni_com.jsonl`), so people do not collide on merge. Reading gathers
every `.jsonl` under `.moai/journal/` plus the old single file and orders them by
timestamp — there is no migration, and a repository that still has only the old
file works exactly as it did.

**A journal file that cannot be read is skipped, not fatal.** Two accounts
sharing one checkout is enough for someone else's `<email>.jsonl` to stand 0600,
and history you can read should not go with it. moai reads the rest, names the
place it could not read and the reason on stderr, and ends non-zero — so a
shortened history never comes back as a successful "no history", and `chmod` is
the fix. The exit code is the one every partial answer shares, so it says "this
run was not whole", not which part; the stderr line is what names the file.
A neighbouring worktree's journal (`show --worktree` reads history where the row
came from) is named the same way but leaves the exit code alone — your own files
are fine, and a neighbour's permissions are not your command failing.

Two habits make recovery cheap, and both are properties of the tool rather than
advice:

- **Reads are lenient, writes are strict** — and strict about *the line being
  written*, not about the whole file. One stale line someone else wrote does not
  block your write, so a bad line never leaves you with no tool.
- **Unknown fields are preserved.** Every write is a full rewrite, so a newer
  binary's fields survive an older binary touching the same file.

## "locked"

A command failed and `--json` says `"code":"locked"`.

The lock is `flock(2)` on `.moai/lock`. The kernel releases it when the process
holding it exits, so there is no such thing as a stale lock to clean up — if you
are seeing this, something is genuinely holding it right now. That is usually a
neighbouring session, a `moai tui` that is open, or a hook.

Wait and retry. Deleting `.moai/lock` does not release anything and only removes
the file the next writer will recreate.

## "broken": a line that will not parse

`moai status` exits non-zero only when the data is broken — warnings never do.
When it does, it names how many lines it could not read.

```sh
git diff -- .moai/issues.jsonl        # what changed it
git log --oneline -- .moai/issues.jsonl
```

Fix the line in place — it is one JSON object on one line — or take the file
from the last commit where it was whole:

```sh
git checkout <commit> -- .moai/issues.jsonl
```

Reading a broken file still works for the lines that parse, so `moai show` and
`moai ready` keep answering while you fix it.

## A conflicted `issues.jsonl`

Lines are sorted by id, so two branches that touched *different* issues still
collide when their lines are neighbours. The merge driver resolves that per
issue:

```sh
moai merge-driver --install   # once per clone; git reads it from config, which is not committed
```

If the merge already stopped with conflict markers:

```sh
git checkout --ours .moai/issues.jsonl    # or --theirs
```

is almost never what you want — it discards one side's issues wholesale. Instead
install the driver and redo the merge:

```sh
git merge --abort
moai merge-driver --install
git merge <branch>
```

What the driver cannot resolve — the same field of the same issue changed two
different ways — comes to you as a normal conflict. One line is one issue, so
that conflict is small enough to read.

**If the driver's recorded path disappears** (it records the absolute path of
the binary that installed it, so installing from a worktree's `target/` and then
deleting that worktree breaks it), git falls back to its own merge and leaves
conflict markers. Re-run `moai merge-driver --install`, or pass `--as <path>`
to record a path that will outlive the worktree.

## A worktree wrote the tracker in the wrong place

`moai` run inside a linked git worktree reads and writes the **main checkout's**
tracker, and says on stderr where it wrote. That is deliberate: editing a
worktree's own `.moai` makes the snapshot conflict at merge time.

If you find changes in a worktree's `.moai/` — from an older binary, or from a
run with `MOAI_HERE=1` — the fix is to move them by hand: they are lines in a
file. Apply the same `moai` commands against the main checkout and drop the
worktree's copy.

## Work that was picked up twice

Two sessions can take the same issue if neither passed `--from`:

```sh
moai mv <id> in_progress --from todo
```

With `--from`, the loser gets a non-zero exit and nothing is written. That is
not an error payload — `--json` returns the usual `moved` / `already` / `missing`
/ `stale` object, and the losing id is in `stale`. There is no `"code":"stale"`;
branch on the `stale` array, not on `code`. Without it, the second write wins and the journal shows both
moves — `moai show <id>` is where you see who did what, and the fix is to agree
and move the line once more.

## Something was closed that should not have been

Nothing is destroyed by a move. `moai mv <id> todo` puts it back, and the
journal keeps every move including the wrong one.

`moai rm` is the exception: it removes the line. The journal records the removal
and git has the file, so:

```sh
git show <commit>:.moai/issues.jsonl | grep '"<id>"'
```

gets the line back; append it and let the next write re-sort the file.

Work you are not doing right now should be `moai defer <id> -m 'why'` rather than
`done` — `--undo` brings back the same line, in the same column, with the same
kind.

## A release went out wrong

The tag is what the release workflow trusts.

- **Tag disagrees with `Cargo.toml`** — the workflow's first job and the pre-push
  hook both stop before anything is built. Run `scripts/bump-version.sh <version>`,
  commit, delete the bad tag locally and on the remote, and tag again.
- **Artifacts are wrong but published** — delete the release and its tag, fix,
  and release a new patch version rather than replacing artifacts under a tag
  someone may already have downloaded. `install.sh` verifies against
  `SHA256SUMS`, so a replaced artifact under an old tag is not a silent
  failure for users, but it is a confusing one.
- **A user reports the installer refusing** — that is the installer working. It
  stops when `SHA256SUMS` is missing, when it has no line for the archive, when
  the digest differs, or when there is no `sha256sum`/`shasum` on the machine.
  Check the release actually carries both files.

## When nothing else fits

The state is two text files under version control. `git log -p -- .moai/` shows
every change anyone made to them, and any commit that had them whole is a
recovery point.
