# Subversion working copies

moai reads changes and diffs from a Subversion (svn) working copy as well as from git. Where a project lives in svn, the three places that show changes work the same way they do for git:

- the [explorer's file window](explorer.md#the-file-window): change marks, **changed files only**, and the diff of the file under the cursor
- the [diff pane](explorer.md#the-diff-pane) over the list, the board and the gantt
- the [web page's](web.md#files-and-diffs) Files tab and its diffs
- `moai diff <id>` and its `--json` form ([the response](json.md#read-an-issues-changes))

Everything here only reads. moai never commits, updates, reverts, locks, or cleans up a working copy.

## What you need

- `svn` 1.9 or later on `PATH`. moai does not check the version; older clients print the added and deleted files' headers in a form it cannot read.
- A working copy: a directory with a `.svn` directory in it or in one of its parents.
- Without `svn` on `PATH` every read fails with `vcs_error.kind` `no_svn` and a line asking you to install Subversion 1.9 or later. See [the kinds](json.md#read-an-issues-changes).

The file window, search and the file contents never need svn. Only the change marks and the diffs do.

## The four scopes

A working copy is one checkout. moai reads the layout of its URL (`svn info`'s relative URL) to decide what `branch` means. The first of `trunk`, `branches` and `tags` met from the repository root decides: `…/trunk[/…]` is the trunk, `…/branches/<x>[/…]` is the branch `<x>`, and a path with none of these words, or a `tags/<x>` copy, is outside the convention. A prefix above it (`proj/trunk`) is kept.

| Scope | Compares | Contacts the server |
|---|---|---|
| `working` | `BASE` against the working copy, untracked files included | No |
| `commits` | each revision whose log message names the issue id (tracker commits excluded) against the revision before it, along this URL's history. A message names an id as a git commit message does: in its first paragraph, or in a `Refs:`, `Closes:` or `Fixes:` line | Yes |
| `branch` | the point where `branches/<x>` was copied from, against the working copy's `BASE` | Yes |
| `whole` | the same fork point against the working copy; when the fork point is unknown, the same as `working` | Only on a branch copied from somewhere |

Where a working copy stands decides what is offered:

| Working copy | `branch` | `whole` |
|---|---|---|
| `…/trunk[/…]` | nothing to compare (as git's main checkout) | as `working` |
| `…/branches/<x>[/…]`, made by `svn copy` | served | served |
| `…/branches/<x>[/…]`, made by `svn mkdir` | not served, with a line saying so | as `working` |
| anything else (`tags/<x>`, no convention) | not served, with a line saying so | as `working` |

Trunk changes merged into a branch since it was copied show up in `branch`: the fork point does not move on a sync merge.

The line is one of:

- `branch is not served: this working copy is not on trunk or branches/<name>`
- `branch is not served: this branch was not copied from anywhere`

It appears in three places only: at the end of `moai diff <id>`'s stderr, in the explorer's diff pane title, and in the refusal of `--scope branch`. It is not in `--json` output, whose keys are those of a git read, and the file window and the web Files tab do not announce it. A branch made by `svn mkdir` is only found out when `branch` is first read.

### Which scopes `moai diff <id>` offers

The rule is the one for git. The working copy is treated as a single checkout: trunk as a main checkout, `branches/<x>` as a linked worktree whose branch is named `<x>`. When `<x>` equals the issue's id, or `worktree-<id>`, the issue (an epic, say) and the members it holds in progress are offered `working` and `branch`; every other issue is offered `commits` only. Changes on trunk that no branch owns are seen in the file window with **changed files only**.

## Git and svn together

A directory can sit inside both a git checkout and a svn working copy. moai reads each independently: git when a `.git` is found in a parent, svn when a `.svn` directory is. When only one is found, that one is used. When both are, **git is the default**, so a `.git` checkout reads exactly as it did before svn was supported.

To read the svn side, choose it, in this order of precedence:

1. `moai diff <id> --vcs svn` (or `--vcs git`)
2. The pick made in the window or tab: `SPC w` in the [file window](explorer.md#the-file-window) gets two more rows, `version control git` and `version control svn`, and the web Files tab gets a **Version control** selector. The pick belongs to that window or tab and is not remembered.
3. `backend` under `[vcs]` in `.moai/config.toml`:

        [vcs]
        backend = "svn"

4. git

Rules for the choice:

- A backend that is not found cannot be chosen. `--vcs svn` where no `.svn` is found fails with `bad_input` (`Version control unavailable: svn; found: git`). A configured backend that is not found is ignored and the one found is read.
- An unknown value under `[vcs]` reads as the default. `moai diff` prints one line to stderr (`Unknown [vcs] backend … in .moai/config.toml; reading with the default`), also with `--json`, where it stays on stderr. Nothing else, `moai status` included, complains about it.
- The selector appears only when both are found. A checkout with only one reads as before, with no extra row or control.
- The list, the board and the gantt's diff pane have no selector and follow the config.

## The revision table and its cache

Subversion can answer "which revisions mention this id" only by asking the server's log. moai keeps the answer, an id-to-revision table, in a cache file in your user cache directory, outside the project and the working copy:

    $XDG_CACHE_HOME/moai/svn/<repository UUID>.json      when XDG_CACHE_HOME is an absolute path
    ~/.cache/moai/svn/<repository UUID>.json             otherwise

- Without a usable location moai reads the log each time and caches nothing.
- The file is safe to delete. It is built again from the server, and a damaged or hand-edited one is thrown away the same way. Nothing in `.moai/` records it.
- It holds every revision's title and its `Refs:`, `Closes:` and `Fixes:` lines, the fork points of branches, and when the table was last found complete. Ids are matched when the table is read, so an id made later is found too. It does not hold patches: each diff reads the server again.
- The first fill reads the log 2,000 revisions at a time and keeps each piece, so an interrupted fill is continued by the next call. Until a fill has completed once, the table answers nothing, since "no revision mentions this id" would be a guess; the read says it is still filling (`timeout`).
- Once complete, the server is asked for new revisions at most once a minute. A revision whose message is edited after the table has read it keeps the message read the first time until the file is deleted.
- A branch's `branch` and `whole` ask for the fork point once and keep it, so the explorer's refreshes do not repeat that call.

## What moai runs

moai calls `svn` with four subcommands only: `info`, `status`, `diff` and `log`. Nothing else can be reached, so a read leaves the working copy and its `.svn` as they were. Every call is non-interactive, has its standard input closed, and on Unix is detached from the controlling terminal, so a password prompt cannot take over the explorer's screen. Output is machine-readable (`--xml`) where svn has it, in English, and in UTF-8.

Because it never prompts, credentials must already be cached. When a server needs a login, run any `svn` command against it once in a terminal and let svn store it.

Property-only changes are not part of a diff: such a file is a modified file with no hunks. A rename is a deletion plus an addition. `.svn/` is listed in the file window as an ignored directory and is never searched.

## Limits and time

| Limit | Value |
|---|---|
| Patch read | 2 MiB; the file list stops there too, and `truncated` is set |
| Changed-file list | 5,000 files |
| Explorer and web reads | 10 seconds; the explorer's work runs in the background and the web answers with a timeout (HTTP 504) |
| `moai diff` | 30 seconds for the whole command, and 30 seconds for each call to the server |

`moai diff` has no deadline of its own for git. For svn, a read that reaches the 30-second limit without the server answering is reported as `offline`; one that stops while the revision table is still being filled is `timeout`, and the pieces read are kept so the next call goes on from there.

When the server cannot be reached the `commits` scope is still offered, but reading its patch fails with `offline`. `working` needs no server and keeps answering, and so does `whole` on a trunk or a branch that has no known fork point. When the revision table cannot be read, those scopes still answer, and the reason ends `moai diff`'s stderr line and the diff pane's title. Once a table has been completed, it answers even when the server is down.

## Known limits

- A diff of a URL against a working copy fails as a whole when a versioned file is missing from disk (`!` in `svn status`), and when the working copy is a subdirectory of a branch that did not exist at the fork point. Both are reported as `failed`, with svn's own words. A list with files left out would be wrong, so none is given.
- With a `.git` and a `.svn` in the same tree, reading as git lists `.svn/**` as untracked. Reading as svn leaves out every unversioned `.git`, directory or file, at any depth.
- A branch deleted and made again under the same name keeps the fork point of the first until the cache file is deleted.
- Not supported: reading renames or copies as such, property changes, `svn:externals`, `tags` as branches, the commits section of `moai show` and the explorer's commit table, and every write.
