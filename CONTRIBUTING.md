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
cargo test                 # 1,500+ tests, a few seconds
cargo clippy --all-targets -- -D warnings
cargo fmt --all --check    # or `cargo fmt --all` to fix
```

**Do not add `--release` to tests.** `[profile.release]` sets `lto = true`, so
every one-file change re-runs the LTO link and a rebuild goes from seconds to
minutes. CI has three jobs: one runs clippy and the tests on the dev profile,
one works out whether anything outside the docs changed, and the third builds
the release binary and checks it against the 15 MB budget. Only the first is a
required check, and the third shows as skipped - not failed - on a pull request
that touched documentation alone.

There are no dev-dependencies, and that is deliberate: `tests/cli.rs` runs the
real binary through `CARGO_BIN_EXE_moai`. A test harness that drags in
dependencies blurs the premise that this is a tool installed into someone else's
repository.

Clippy runs with `-D warnings`. Lints this project deliberately carries are
listed under `[lints.clippy]` in `Cargo.toml`, each with the reason it is there.
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
mixed with a change to the code. Note that `rust-version` in `Cargo.toml` now
records the minimum rather than proving it — nothing builds on that version.

The whole repository was formatted in one commit. `git blame` can step over it
so that it points at the commit that actually wrote each line, but git only
reads that list from config, and config is not committed — so turn it on once
per clone, the same as the `.moai` merge driver:

```sh
git config blame.ignoreRevsFile .git-blame-ignore-revs
```

A clone that skips this is no worse off than before; `blame` just stops at the
formatting commit.

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
  from anywhere but `develop` — a ruleset holds the first, and a step in
  `ci-gate` holds the second, because GitHub has no rule that pins the source
  branch of a pull request. `main` carries one commit per release that way, and
  its history reads as the distance between two releases.

## Releasing

```sh
scripts/bump-version.sh 0.2.0                     # Cargo.toml, Cargo.lock, CHANGELOG.md
git commit -m "chore(release): 0.2.0" -- Cargo.toml Cargo.lock CHANGELOG.md
git push origin develop
gh pr create --base main --head develop --title "v0.2.0"   # merge it, no squash
git fetch origin && git tag v0.2.0 origin/main
git push origin v0.2.0
```

**The tag goes on the merge commit in `main`, not on the tip of `develop`.**
`main` is what a release is cut from, and a tag on `develop` would build
something that is not what `main` holds. **Merge that pull request, do not
squash it** — `moai show <id>` finds an issue's commits by the id in their
subject, and a squash folds 400 subjects into one body where nothing reads them.

The tag is what the release workflow trusts, so the pre-push hook and the first
job of the workflow both check the tag against `Cargo.toml` before anything is
built. The script deliberately neither commits nor tags: if one hand wrote both
values there would be nothing left to check them against.

For the very first release the bump script has nothing to move — `Cargo.toml` is
already at `0.1.0` — so rename the `[Unreleased]` heading in `CHANGELOG.md` to
`## [0.1.0] - <date>` by hand, add a fresh empty `[Unreleased]` above it, and tag.

A `v*` tag builds `x86_64-unknown-linux-musl` and `aarch64-apple-darwin`,
publishes both archives with `SHA256SUMS` and `install.sh`, and takes the release
notes from that version's section of `CHANGELOG.md`.

## When something breaks

`docs/recovery.md` covers the tracker files: a stuck lock, a conflicted
snapshot, lines an older binary may have dropped, and a release that went out
wrong.
