# Contributing

## Once per clone

```sh
moai merge-driver --install     # resolve issues.jsonl per issue
scripts/install-git-hooks.sh    # pre-push and prepare-commit-msg
```

Neither is committed — git reads merge-driver commands from config, and hooks
live outside the working tree — so a clone that skips these behaves exactly as
before. They are a safety net, not a rule.

The installer puts in two shims:

- `pre-push` refuses a tag that disagrees with `Cargo.toml`
- `prepare-commit-msg` adds `Executed-By: Name <email>` when `MOAI_ACTOR` is set,
  so `git log` shows which commits an agent ran. The tracker's journal already
  records who, but that lives in `.moai`; a trailer is the cheapest way to say
  the same thing where git itself keeps it. Merge and squash commits are left
  alone, so is an amend that already carries the line, and so is a message you have
  not written yet — a plain `git commit`, and a `-v` one whose diff sits below the
  scissors line, both get their template back untouched.

Each shim keeps a hook that is already there: it moves it to
`<name>.moai-before` and calls it first, so lefthook, husky or your own script
keeps working. `scripts/install-git-hooks.sh --uninstall` puts it back.

The shims are thin, and they block in exactly two cases: the hook that was there
before one of them fails, or a tag disagrees with `Cargo.toml`. Missing tools, a
missing checkout, a check that runs long, a trailer that could not be written —
all pass. Pushing and committing should not be blocked by the tooling's own
circumstances, and the release workflow measures the tag again where it actually
matters.

## Build and test

```sh
cargo build --release      # target/release/moai
cargo test --workspace     # 1,500+ tests, a few seconds
cargo clippy --all-targets --workspace -- -D warnings
cargo fmt --all --check    # or `cargo fmt --all` to fix
```

The repository is a Cargo workspace: the root package is `moai`, and
`crates/moai-gantt` is a member. A bare `cargo test` or `cargo clippy` in the
root covers `moai` alone, so pass `--workspace` to check what CI checks.

**Do not add `--release` to tests.** `[profile.release]` sets `lto = true`, so
every one-file change re-runs the LTO link and a rebuild goes from seconds to
minutes. CI has five jobs: one runs clippy and the tests on the dev profile,
one works out whether anything outside the docs changed, one builds the crate
with the `rust-version` written in `Cargo.toml`, one runs clippy and builds the
tests on Windows without running them, and the last builds the release binary
and checks it against the 15 MB budget. The first and the last are the checks
that branch protection on `main` requires; the other three are not. The last
three skip on a pull request that touched documentation alone and show as
skipped - not failed - and a skipped required check still counts as passing,
so a docs-only pull request is not held up.

There are no dev-dependencies, and that is deliberate: `tests/cli.rs` runs the
real binary through `CARGO_BIN_EXE_moai`. A test harness that drags in
dependencies blurs the premise that this is a tool installed into someone else's
repository.

Clippy runs with `-D warnings`. Lints this project deliberately carries are
listed under `[workspace.lints.clippy]` in `Cargo.toml`, each with the reason it is there.
Add the reason when you add a line — an `allow` without one cannot be judged
later.

`cargo fmt --all --check` runs in CI, before clippy and the tests. The width is
set in `rustfmt.toml`, which also records why that value and not the default.

**The toolchain is pinned in `rust-toolchain.toml`**, so rustup builds this
repository — and formats and lints it — with the version written there, and your
`cargo fmt` gives what CI sees. Without the pin CI ran on whatever stable the
runner shipped that week, and rustfmt moving a line it has never touched turned
someone else's pull request red. Bumping the pin is its own commit: change the
channel and carry the `cargo fmt --all` it causes in the same commit, never
mixed with a change to the code. `rust-version` in `Cargo.toml` is a separate
claim, and the `msrv` job proves it: that job reads the line and builds the
crate with exactly that toolchain, so bumping the line moves what CI walks.

The whole repository was formatted in one commit. `git blame` can step over it
so that it points at the commit that actually wrote each line, but git only
reads that list from config, and config is not committed — so turn it on once
per clone, the same as the `.moai` merge driver:

```sh
git config blame.ignoreRevsFile .git-blame-ignore-revs
```

A clone that skips this is no worse off than before; `blame` just stops at the
formatting commit.

## The web page

`moai web` serves a page built from `web/`, a TypeScript project (the Saycode
panel, taken as a base). The built bundle is committed under `src/web/dist/` and
compiled into the binary, so a plain `cargo build` needs no node.

- Change `web/`, then rebuild the bundle with node 22: `scripts/gen-web.sh`.
  Commit the source and the bundle in the same commit
- `cargo test` runs `tests/web_bundle.rs`, which fails when the committed bundle
  was not built from the sources in `web/` (it compares a hash of them, so it
  needs no node). CI's web job rebuilds it on a clean machine and catches the
  same drift, plus a bundle built with another node
- The page's own tests: `cd web && npm ci && npm run typecheck && npm test`
- The page reads what moai answers and decides nothing itself, so its test data
  is this binary's answers: `cargo build && MOAI=target/debug/moai
  scripts/gen-web-fixtures.sh` rewrites `web/tests/fixtures/moai/`.
  `the_web_fixtures_match_the_binary` fails when a change to a command's JSON
  leaves them behind
- `web/UPSTREAM.md` says which Saycode panel commit the page came from and what
  was changed since; read it before pulling a newer panel in
- The server side is `src/web/`; its error codes live in `src/fail.rs`

## Developing on Windows

Windows is a release target (`x86_64-pc-windows-msvc`, and
`aarch64-pc-windows-msvc` cross-built on the x64 runner), and you can build, test
and keep developing on it.

`scripts/windows-dev.ps1` gets a clean machine ready and is safe to run again:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\windows-dev.ps1
```

- It looks for Git for Windows, the Visual Studio C++ build tools (the `ring`
  crate builds C with `cl.exe`) and rustup. **It installs nothing**: what is
  missing is printed as a `winget` command for you to run (or, for a Visual
  Studio that is already installed, its installer's `modify` command), and it
  stops there
- On an ARM64 machine it looks for the ARM64 build tools and `clang` instead —
  the host target there is `aarch64-pc-windows-msvc`, and `ring` builds its C
  for that target with `clang` from `PATH`. Visual Studio's clang component
  does not put itself on `PATH`; when it stands, the script names the directory
  to add. It reads the machine's architecture, so an x64 PowerShell running
  emulated there gets the same check
- rustup reads `rust-toolchain.toml` and fetches the pinned toolchain for the
  host target by itself; the script only checks `cargo --version`
- It runs `cargo build --release`, `cargo clippy --all-targets -- -D warnings`
  and `cargo test --no-fail-fast`, prints one line each (plus the
  `test result: FAILED` line of every test binary that failed), and keeps the
  full output in `target\windows-dev\*.log`. `-SkipTest` stops after clippy, `-SkipBuild`
  only checks the tools
- It installs the merge driver with the `moai.exe` it just built
  (`merge-driver --install --as <that path>`), so keep that build where it is or
  install again with a path that stays. `-SkipDriver` leaves it out
- It never edits `PATH`, the registry or anyone's config

`.ps1` files in this repository are ASCII only — Windows PowerShell 5.1 reads a
script without a BOM in the system code page, and `install_ps1_is_ascii` holds
the line.

### What to check on a real machine

The container that builds this repository cannot run these. Check them on
Windows and leave what you saw on the issue with `moai note <id> '…'` — the
command, what it printed, and the Windows and PowerShell versions
(`$PSVersionTable.PSVersion`):

- `moai-j9gv.4c1` — which shell Claude Code starts hooks with; whether Git's
  `sh` can exec `'C:/…/moai.exe'` as the merge driver records it; whether the
  hooks' `${TMPDIR:-/tmp}` is writable
- `moai-j9gv.3fl` — `moai update --dry-run`, one real upgrade, and that
  `moai.exe.old` is gone after the next install
- `moai-j9gv.ukd` — installing with `install.ps1`, and that a tampered zip or a
  missing `SHA256SUMS` is refused
- `moai-j9gv.ae7` — the test failures counted under wine (about 150); count them
  again with `cargo test --no-fail-fast` (the script runs that) and note the
  summary lines
- `moai-l3pp` — `SPC n` with `code --wait`, and with Git's `nano.exe` from
  PowerShell and from Git Bash, on a profile whose temporary directory holds a
  space or `'`: the jot comes back. A temporary directory on a share
  (`\\server\…`) with `code --wait` is a known gap (review `moai-j9gv.8oh`)
- `moai-uwzg.q1j` — the ARM64 build is cross-built on the x64 runner and never
  run there: on ARM64 Windows, `install.ps1` from the native PowerShell installs
  `aarch64-pc-windows-msvc`, and that `moai.exe --version` starts on a machine
  without the Visual C++ redistributable

## If you change a command's help

`docs/cli.md` is generated from `--help`, and a test compares the two. When you
change help text, or add or remove a command, regenerate and commit it:

```sh
cargo build --release && scripts/gen-cli-docs.sh
```

A reference written by hand drifts from the binary, and a reference that has
drifted is worse than none at all — a reader trusts it and types what it says.
So the words live in one place and that file is a copy of them.

## What a change looks like

Work is tracked in `.moai/issues.jsonl`, in this repository, with this tool.
These commands write to it, so they are for maintainers with push access — from
a fork, describe the issue in the pull request instead and a maintainer will
file it:

```sh
moai status                  # start here
moai ready                   # what can be picked up
moai mv <id> in_progress --from todo
```

Put the issue id in the **commit subject**: `fix(view): stop wrapping at 80 (<id>)`.
`moai show <id>` finds a commit by reading ids out of subjects — nothing is
stored, so a rebase or a squash cannot make it stale. In the body, only lines
starting with `Refs:`, `Closes:` or `Fixes:` count.

Write commit messages about **why**, not what. The diff already says what. If a
decision should not be reverted later, the reason belongs in the message.

One commit is one thing. A change made in response to review is its own `fix:`
commit, and a review point you decided not to act on gets a sentence saying why.

## Pull requests

- Say what changed, why, and how you verified it.
- Do not include changes to `.moai/` — that directory is this repository's own
  tracker, and CI rejects external pull requests that touch it. Describe the
  issue in the pull request instead and a maintainer will file it.
- New commands are caught by the tests that sweep every command for `--json`
  support, so add the command and let the test tell you what it wants.
- **Open it against `develop`.** `main` takes no direct push and no pull request
  from anywhere but `develop` — branch protection holds the first, and a step in
  `ci-gate` holds the second, because GitHub has no rule that pins the source
  branch of a pull request. `main` carries one commit per release that way, and
  its history reads as the distance between two releases.

## Releasing

```sh
git -C .worktrees/milestone-<id> merge --no-ff develop -m "merge: take develop into milestone/<id>"
cargo test                                        # in .worktrees/milestone-<id>
git merge --no-ff milestone/<id> -m "merge: <milestone title> (<id>)"   # in the root, on develop
moai archive --dry-run                            # preview closed bundles eligible to move
moai archive                                      # refresh .moai/archive/<year>.jsonl
git add .moai/archive                             # a new year's file is not tracked yet
git commit -m "chore(tracker): archive closed bundles" -- .moai/   # the journals' notes too
scripts/bump-version.sh --next                    # prints 0.2.0 and why; writes nothing
scripts/bump-version.sh auto                      # Cargo.toml, Cargo.lock, CHANGELOG.md
git commit -m "chore(release): 0.2.0" -- Cargo.toml Cargo.lock CHANGELOG.md
scripts/release-post/compose.py v0.2.0 --out release-posts/v0.2.0   # the Discord post, from the section auto just opened
release-posts/v0.2.0/make.sh                      # a screen changed? its pictures, copied from v0.24.0/make.sh
scripts/release-post/post.py release-posts/v0.2.0 --preview /tmp/post.html   # look at it before it goes out
git add release-posts && git commit -m "docs(release-posts): v0.2.0" -- release-posts
git push origin develop
gh pr create --base main --head develop --title "v0.2.0"   # merge it, no squash
# an admin approves "tag on merge" in Actions (environment release);
# v0.2.0 is tagged on main's merge commit and the release workflow runs:
# it builds, releases, then posts release-posts/v0.2.0 to Discord
git worktree remove .worktrees/milestone-<id>     # once the release is out
git branch -d milestone/<id>
rm -rf /tmp/cargo-target/milestone-<id>
```

**A milestone's work waits on its own branch until the release** (moai-nvju,
2026-10-08). Work inside a milestone branches from `milestone/<id>` and merges
back into it, not into `develop`. The branch is raised from the local `develop`
when the milestone's first epic is sent, by the worker that receives it, and it
is checked out in a long-lived worktree at `.worktrees/milestone-<id>` because
the root stays on `develop`:

```sh
git worktree add -b milestone/<id> .worktrees/milestone-<id> develop   # no branch yet
git worktree add .worktrees/milestone-<id> milestone/<id>               # branch, no worktree
git worktree add -b worktree-<epic> .worktrees/<epic> milestone/<id>    # an epic of it
git -C .worktrees/milestone-<id> merge --no-ff worktree-<epic> -m "merge: … (<epic>)"
```

The epic's merge runs in the milestone worktree, after checking that it still
stands on `refs/heads/milestone/<id>`; a conflict there is aborted, resolved in
the epic's worktree after taking the milestone branch in, and merged again. Work
outside every milestone — a `p0` hotfix — branches from `develop` and merges
into `develop`, as before. The milestone branch takes `develop` in **once, at
the release**, before `auto` — the first three lines of the block above, left
out when the release has no milestone branch — so
the changelog `auto` reads holds the milestone's entries, and anything that
landed on `develop` meanwhile, a `p0` included, ships with it. Conflicts are
resolved in the milestone worktree, never in the root. If an epic needs a fix
that landed on `develop` earlier, the milestone branch takes `develop` in at
that moment instead of waiting. After the release the milestone worktree and
branch are removed.

**Nothing is rebased or squashed on the way in** — epic branches and the
milestone branch go in with `git merge --no-ff` as they stand. `moai show <id>`
finds an issue's commits by the id in their subject and a squash folds them
away; a review's fixes stay as their own `fix:` commits; and a rebase changes
the merge hashes that `Report-checked:` notes point at. Rewording a subject on
your own unshared branch before its review starts is fine; never during one.

**Each release is announced on Discord** (moai-hwl4, 2026-10-25). The post lives in
`release-posts/<version>/` and goes in before the tag, so the tag carries it:

    post.md            the message — big changes first, then fixes; one line each
    media/01-keys.png  then one message per picture or clip, in name order
    media/01-keys.md   the line under that picture (optional)
    make.sh            how the pictures were made, so they can be made again

`compose.py` writes `post.md` from that version's changelog section: an entry's bold
lead, or its first sentence without the aside, six per group and the rest left to the
release notes. Edit it if it reads badly — it is in English, like the Discord server.
**A release that changes what the explorer or `moai web` shows carries a picture**
— a screenshot, a clip, or both — and it is made before the tag, since the tag
posts what is committed (moai-r96m, 2026-10-25: v0.24.0 announced the new gantt
in text alone and was posted again). `compose.py` says so in one line on stderr
when a big change names the explorer, `moai tui`, `moai web` or a `SPC` key and
`media/` holds no picture; it only tells, and the exit code stays 0.
Record the scene with `scripts/site-demo` and take it with `shot.mjs`, which draws
the frame the homepage uses and puts a box around what changed — `--mark <text>`,
`--line <text>` for a whole line, `--select <css>` on a web page. A `-o` ending in
`.webm` takes a clip of the scene instead. `release-posts/v0.24.0/make.sh` (a TUI
scene with a clip) and `release-posts/v0.23.0/make.sh` (a shell scene) are the
patterns to copy. Playwright is not a dependency of this repository; pass it in with
`NODE_PATH`, as `scripts/site-demo/record-web.mjs` does.

The release workflow posts it once the GitHub release is out (`discord.yml`, skipped
for a pre-release such as `-rc.1`). With no `post.md` committed it posts the text alone,
composed from the changelog. The webhook is the repository secret
`DISCORD_WEBHOOK_URL` (Settings → Secrets and variables → Actions); without it the
job prints what it would have sent and stays green. To post a release again, or one
that went out before this — v0.23.0 was the first — run it by hand:

```sh
gh workflow run discord.yml -f version=v0.2.0                # post it
gh workflow run discord.yml -f version=v0.2.0 -f dry_run=true   # print what it would post
gh workflow run discord.yml -f version=v0.2.0 -f start=3        # a run that stopped after message 2
```

A run that fails part way says how many messages went out and where to start again,
so a retry does not post the first ones twice.

Before `auto`, the `moai-wiki` skill's sweep can be called once: it walks the work
closed since the last tag and lists the wiki pages it left behind. It is not a
gate — nothing waits on it, and a release goes out whatever it finds.

**The changelog picks the number.** `auto` reads the headings under
`[Unreleased]` that hold entries. Before 1.0, `Fixed` and `Security` alone make a
patch release, and any `Added`, `Changed`, `Deprecated` or `Removed` makes a minor
one — a change in behaviour is not a fix. So the call you make is which heading an
entry goes under, and the script counts the number from there: two people, or two
agents, releasing the same changelog get the same version. A heading it does not
know, or an `[Unreleased]` with no entry under a heading, stops it with nothing
moved. A lead paragraph above the first heading — 0.1.2 gathered its breaking
changes there — is left out of the count, so every entry goes under a heading
as well. It never picks a major version — `1.0.0` is given by hand — and
after 1.0 `auto` stops as well, because which removals and behaviour changes count
as breaking has not been decided yet. `minor`, `patch` or an explicit version
still go through where the rule does not fit.

**The tag goes on the merge commit in `main`, not on the tip of `develop`.**
`main` is what a release is cut from, and a tag on `develop` would build
something that is not what `main` holds. **Merge that pull request, do not
squash it** — `moai show <id>` finds an issue's commits by the id in their
subject, and a squash folds 400 subjects into one body where nothing reads them.

**Nobody types the tag; an admin approves it** (moai-i5so, 2026-10-23). When a
pull request from `develop` is merged into `main`, `tag-on-merge.yml` first
reads the merge commit. If the tag `v<Cargo.toml version>` already exists, the
merge did not bump the version, and nothing is asked. If `CHANGELOG.md` has no
section for that version, the run stops before anyone is asked. Otherwise it
waits on the `release` environment. Once an admin approves, it tags the merge
commit and starts `release.yml` and `smoke.yml` on that tag. It starts them by
`workflow_dispatch` because a tag pushed with `GITHUB_TOKEN` does not trigger
other workflows' `push: tags`, so the release workflow itself is unchanged.

The approval lives in the repository settings, not in this file: under
Settings → Environments → `release`, the admins are the required reviewers.
Leave its deployment branches unrestricted. The job runs on a `pull_request`
event, so its ref is not `refs/heads/main`, and a rule naming `main` keeps the
job from starting. If that environment is missing or loses its rule, GitHub runs
the job without asking. So the job's first step reads the run's approvals and
stops unless an admin approved for `release`. An empty setting means no tag, not
an unapproved release.

The tag is what the release workflow trusts, so the pre-push hook and the first
job of the workflow both check the tag against `Cargo.toml` before anything is
built. The script deliberately neither commits nor tags: if one hand wrote both
values there would be nothing left to check them against. A tag made from
`Cargo.toml` always passes that check, so on the automatic path three other
things carry it. An admin's approval stands where a person read the version.
That version's section must already be in `CHANGELOG.md`, which `auto`
opened, or the run stops. And the tag must not exist yet.

If the approval was rejected or the run failed before tagging, tag by hand as
before. The pre-push hook and the guard still check it, and the push itself
starts `release.yml` and `smoke.yml`:

```sh
git fetch origin && git tag v0.2.0 origin/main
git push origin v0.2.0
```

If the run made the tag but a workflow did not start, do not push the tag again.
It already exists. Start the workflows on it instead:

```sh
gh workflow run release.yml --ref v0.2.0
gh workflow run smoke.yml --ref v0.2.0
```

For the very first release the bump script has nothing to move — `Cargo.toml` is
already at `0.1.0` — so rename the `[Unreleased]` heading in `CHANGELOG.md` to
`## [0.1.0] - <date>` by hand and add a fresh empty `[Unreleased]` above it. The
merge into `main` then tags it like any other release.

A `v*` tag builds `x86_64-unknown-linux-musl`, `aarch64-apple-darwin`,
`x86_64-pc-windows-msvc` and `aarch64-pc-windows-msvc`, publishes the two
`.tar.gz` archives and the two Windows `.zip` archives with `SHA256SUMS`, `install.sh` and `install.ps1`, and takes the release
notes from that version's section of `CHANGELOG.md`. The Windows builds link the
C runtime statically (`.cargo/config.toml`). A new target is a row in the
release matrix, a line in one installer's platform table, and a
`target_os`/`target_arch` pair in `latest::SERVED` and the tests' `served()` —
`the_install_table_and_the_release_matrix_name_the_same_targets` and
`served_names_the_targets_the_release_matrix_builds` fail until they all agree.
`install.ps1` stays ASCII-only, because Windows PowerShell 5.1 reads a script
without a BOM in the system code page.

## When something breaks

`docs/recovery.md` covers the tracker files: a stuck lock, a conflicted
snapshot, lines an older binary may have dropped, and a release that went out
wrong.
