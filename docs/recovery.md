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

`issues.jsonl`, the [snapshot](glossary.md#snapshot), is what every command
reads. The [journal](glossary.md#journal) records `create`, `status`, `note` and
`rm`; field edits are not recorded. If the journal is lost, you lose history, not
state.

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

**`--json` says which row paid for it.** `show <id> --json` and the list carry a
`journal_error` array beside the row whose history came up short, shaped like
`commits_error`: `kind` (`permission`, `outside` or `failed`) is what a machine
branches on, `said` is the same line stderr prints, naming the file to `chmod`.
`outside` is a journal link that points out of the checkout or into `.git/` — moai
follows a link in a file the repository holds only inside its own checkout, so
the fix is that link, not a permission. Treat a `kind` you do not know as
`failed`. A row carries
only its own root's failures, so a neighbour's locked journal never lands on your
rows. The key is absent when nothing was skipped — `journal` is always there, so
an empty `journal` with no `journal_error` beside it is "no history", and the
same empty array with the key beside it is "could not be read".

**A journal line cut short stays as it is.** A full disk or a crash in the middle
of an append leaves the last line without its newline. The next append puts the
newline back before it writes, so the new line stands on its own and the cut one
is left alone; the history skips it. A whole line that an older moai wrote
straight onto a cut one is read back and shown. `moai show --removed` is the one
place a cut line can hide part of the answer, since the journal is all it reads:
a cut line that may have held a removal — judged by the kind the line names
itself — is named there on stderr by file and line, and the run ends non-zero.
With `--since` that happens only when the line's own stamp falls in the range or
cannot be read. moai never rewrites the journal, so the fix is by hand: look at
what the line held, delete it, and commit.

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

When the line is not worth keeping — most often the broken twin of an issue
that also stands whole, left by a hand-resolved merge — remove it inside the
tool. `moai show` names it by line number:

```sh
moai rm --line 812                          # shows the line and its hash
moai rm --line 812 --yes --match 1a2b3c4d   # removes it
```

Only an unreadable line is removed that way; any other line is refused, and
the refusal lists the unreadable lines as they stand now, each with the id it
carries. Numbers move whenever a row comes or goes — an earlier `rm --line`
included — so `--yes` is bound to the line you saw: the preview prints a short
hash of that line and the whole command to type, `--yes` needs that hash in
`--match`, and when the line now at that number does not match it nothing is
removed and the refusal names the number the line you saw stands at now. Each
removal needs its own preview — the hash also counts identical copies of the
line, so typing the same command again after it went is refused. The removed
line is printed, goes into the journal's `rm` entry (up to 64KB) and `--json`
hands it back whole.

A row written by a newer moai — a `kind` this binary does not know — is not
broken. It reads again once this binary is upgraded, so leave it where it is.

Reading a broken file still works for the lines that parse, so `moai show` and
`moai ready` keep answering while you fix it.

## "broken": the snapshot or config is not a file moai reads

Commands in that repository stop — `moai show`, `moai ready` and `moai rm --line`
included — with one line naming `.moai/issues.jsonl` or `.moai/config.toml`, and
`--json` says `"code":"broken"`. The file is a link that leaves the checkout or
goes into `.git/` (a link whose target is missing counts too), or it is not a
regular file (a FIFO, a socket, a device, a directory); the line says which, and
where a link points. moai follows a link in a file the repository holds only
inside its own checkout — the directory that holds `.moai` — so a committed
`issues.jsonl -> /dev/zero` cannot make it read without end. A link that stays
inside (`-> ../data/issues.jsonl`) reads as before. Where several projects are
shown at once (`moai status` outside a repository, `moai project ls`, the
explorer), that project is named as one that cannot be read and the rest go on;
under `--json` its entry is `"state":"unreadable"` with the same
`"code":"broken"` beside the reason. `moai prime` does not stop either — its
exit code is always 0 — so it names the reason on the first line of its page
instead of sending you to `moai init`, and under `--json` it carries the same
code as `"tracker_error":{"code":"broken","said":…}`.

There is no line to remove; the fix is the file. If it changed only in your
checkout, put the committed one back (a directory there goes with everything in
it, so look first):

```sh
git checkout -- .moai/issues.jsonl    # or .moai/config.toml
```

If the link itself was committed, that brings the same link back. Take the file
from the last commit where it was a regular file, and commit it:

```sh
git log --oneline -- .moai/issues.jsonl
git checkout <commit> -- .moai/issues.jsonl
```

Do not copy what the link points at into its place without looking — it may be
`/dev/zero` or a file under `/proc` that never ends.

## "broken": the lock is not a file moai locks

Every write stops — `moai add`, `mv`, `note` and the rest — with one line naming
`.moai/lock`, or the `lock` beside the file a linked `.moai/issues.jsonl` points
at, and `--json` says `"code":"broken"`. Nothing was written, and reads keep
working. moai takes its lock only on a regular file it makes itself, never
through a link — not even one that points inside the checkout — and only in a
directory that stays inside the checkout and outside `.git/`. The line says
which of those it is.

The lock holds nothing, so remove whatever stands there and run the command
again (a directory goes with everything in it, so look first):

```sh
rm .moai/lock
```

`moai init` puts `.moai/lock` in `.gitignore`, so a link there that keeps coming
back after a checkout was committed anyway. Take it out of the repository:

```sh
git rm --cached .moai/lock
git commit -m 'Stop tracking the moai lock'
```

If the line says the lock points out of the checkout or into `.git/`, it is the
`.moai` directory itself that is a link — the fix is that link. If it says
`.moai/issues.jsonl` points at a lock, the snapshot is the link to fix, the same
way as in the section above.

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

`moai` run inside a linked git [worktree](glossary.md#worktree) reads and writes
the **main checkout's** tracker, and says on stderr where it wrote. That is deliberate: editing a
worktree's own `.moai` makes the snapshot conflict at merge time.

If you find changes in a worktree's `.moai/` — from an older binary, or from a
run with `MOAI_HERE=1` — the fix is to move them by hand: they are lines in a
file. Apply the same `moai` commands against the main checkout and drop the
worktree's copy.

## Work that was picked up twice

Two sessions can [pick up](glossary.md#pick-up) the same issue if neither passed
`--from`:

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
and git has the file, so, with `<commit>` any commit from before the removal:

```sh
echo '<commit>:.moai/issues.jsonl' | git cat-file --batch --follow-symlinks | grep '"<id>"'
```

gets the line back; append it and let the next write re-sort the file.

This reads the file whether `.moai/issues.jsonl` is a plain file or a link to a
file elsewhere in the checkout (a linked tracker). `git show
<commit>:.moai/issues.jsonl` only does the first: for a link it prints the link's
target text and nothing else, so the `grep` finds nothing and says nothing. A link
that points outside the repository comes back as `symlink` and the target path —
git holds no copy of that file, so read it there.

Work you are not doing right now should be [deferred](glossary.md#deferred) —
`moai defer <id> -m 'why'` rather than `done` — and `--undo` brings back the same
line, in the same column, with the same kind.

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
