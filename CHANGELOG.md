# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and this project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

`scripts/bump-version.sh <version>` moves whatever has accumulated under
`[Unreleased]` into a new release section and leaves `[Unreleased]` empty for the
next one. It does not commit and it does not tag — see `CONTRIBUTING.md`.

## [Unreleased]

### Changed

- The release check says *why* it could not ask. The version line still reads as
  one of four, but the fourth now carries the reason in parentheses — no network,
  timed out, rate limited, a server error, TLS failed, unreadable answer, odd
  tag, the call failed — because what a person can do about it differs per reason:
  waiting fixes a rate limit, a proxy that swaps certificates never will. Nothing
  is blocked and the exit code never changes.
- `latest.toml` also records **why** the last call could not answer, so the reason
  stands for the whole day rather than only on the run that asked it. A run that
  heard a tag clears it: the file holds one answer, and a tag is that answer.
- `latest.toml` also records **where** the answer came from, and an answer is only
  reused for the place it came from. Pointing `MOAI_API_URL` at a mirror once no
  longer makes that mirror's tag stand as the real release for a day, and the
  reverse. A file written before this key is read as before, and asked again once.
- `check` under `[update]` is read in the same pass as the rest of your user
  config, so a value that is not `true` or `false` now says so in the explorer's
  notices instead of being ignored in silence. It still does not block anything,
  and anything that is not `false` leaves the check on.
- A stamp a little ahead of this machine's clock no longer forces a fresh ask.
  Two machines sharing one config directory with clocks seconds apart made one of
  them knock on GitHub every single run.
- Unfolding a parked thought teaches the two steps that used to come after it and
  were written down nowhere: hang the running milestone on the epic `promote` made
  (it has no flag for one and does not carry over the idea's), and copy the idea's
  body onto that epic so `moai show <epic>` can say why those issues are one
  bundle. The milestone line is the same one the supervisor's brief uses, so the
  two texts cannot drift apart.
- The worker brief holds a running review's worktree. While `/code-review --fix` is
  going, its branch and working tree are left alone — the fixes sit there
  uncommitted, and `reset --hard`, `rebase` or `commit --amend` throw them away —
  and when it returns, whatever subagent it left running is stopped before anything
  else touches the tree. Both are lines to read: nothing is blocked.
- The brief carries the five places a review keeps finding — a doc block taken over
  by an item inserted above it, a test that cannot go red on a revert, a value set
  up in only one of the several paths that build it, comments that no longer say
  what the code does, and a read path that newly opens something it never opened.
  They go on top of the angle rather than in place of it.
- Closing an epic looks at whether that epic's line stands in the CHANGELOG section
  for the release being prepared. The window that closes an epic is the only one
  that knows what was taken out as well as what went in. Nothing checks it.
- The review grades gained the case they were missing: a worktree that carries
  members of two epics is measured as one epic and then raised one more step.

### Fixed

- The release check no longer follows a redirect down to plaintext `http`. A call
  that starts on `https` is refused rather than downgraded, on every hop. A call
  you pointed at a plaintext mirror yourself still works — that one is your choice.

## [0.1.1] - 2026-09-22

### Added

- `moai tui` says whether a newer release is out. It asks GitHub once a day, and
  the version line in the header reads as one of four: a new release, the latest,
  ahead of the latest (a build from source), or not asked. It only asks where a
  person is watching — `--json`, a pipe and anything that is not a terminal never
  ask, so a machine running agents does not knock on the outside every run. The
  answer and when it was asked are held in `latest.toml` beside your user config,
  and `check = false` under `[update]` there, or `MOAI_NO_UPDATE_CHECK=1`, turns
  it off. Nothing is blocked and the exit code never changes. The HTTPS client
  it needs is most of why the release binary grew this release — it now measures
  7.6 MB of the 15 MB budget, where both numbers count as `scripts/check-size.sh`
  does (8,001,296 bytes of 15,728,640).
- Times on screen are drawn in the reader's timezone. They stood in UTC alone,
  so a reader in Seoul saw every stamp nine hours out. What is stored and what
  `--json` carries are still UTC — only the letters a person reads move. The CLI
  follows the system (`TZ`, then `/etc/localtime`); the explorer takes a name
  picked with `SPC o t` and writes it as `timezone` under `[tui]`. The release is
  a static musl build, so a machine with no `/usr/share/zoneinfo` (Alpine,
  scratch) falls back to UTC and says so in one line, blocking nothing.
- The journal is one file per person, named from the email
  (`.moai/journal/raven_buzzni_com.jsonl`). Several files is the normal shape —
  the reader merges all of them, so an email that changes only adds one. The old
  single `.moai/journal.jsonl` is read as one of those files and nothing is
  migrated. Where the email is not known, nothing is written at all: a file whose
  purpose is history is worse with unowned lines in it than with one question
  asked.
- `moai init` plants the merge driver itself. Until now it wrote the
  `merge=moai` name into `.gitattributes` and left the command that name points
  at to be installed by hand — half of something that does not work in halves.
  `--no-driver` leaves `.git/config` alone, and `--check` answers in one word and
  writes nothing. A repository that decides against per-issue merging records
  that in git's own vocabulary (`-merge` on the snapshot path), and both `init`
  and the `moai status` notice read it as a decision instead of putting the line
  back.
- The explorer's detail pane has a side: `SPC o d` cycles it through right,
  bottom, left and top, `Ctrl-w j` and `Ctrl-w k` move between the panes while it
  is split top and bottom, and the epic, milestone and blocked lines inside it
  carry the same overlap marks the list rows do.
- `show --json`, for one issue and for a list, carries `journal_error` beside a
  row whose history came up short — `kind` (`permission`, `failed`) to branch on
  and `said` naming the file to `chmod`, the same shape as `commits_error`. Until
  now the exit code was the only signal, and it is shared with every other partial
  answer, so a machine could tell "this run was not whole" but never which row.
  A row carries only its own checkout's failures.
- The epic and milestone bars name how many of their members are deferred
  (`1/2  deferred 1`), on the board, in `moai show <group>` and in
  `status --json`. The denominator still counts them — the bar is "of what was
  promised, how much is done" — so without the count beside it the reader sees a
  number that never drops and reads the tally as broken.
- `--json` always carries `kind` and `priority`, including on the rows whose
  snapshot line leaves them out because they hold the default. What the file
  omits and what the contract omits are two different things — keys that can
  genuinely be absent (`epic`, `milestone`, `deferred_at`) stay absent.
- Refusals carry data, so they translate. `MOAI_LANG=ko` reached the screens in
  0.1.0 but not the messages that say no; the write path and its validation, the
  config reader, the plan parser, git, resolving a person, the issue commands,
  entering the explorer, and the `skill` and `merge-driver` notices now all speak
  through `i18n`.
- The supervisor skill treats a running milestone as the gate before it hands
  work out. Work outside one is not assigned, and to assign it you attach the
  milestone to its epic — which is the same ordering `moai ready` already used.
- `rust-toolchain.toml` pins the toolchain that builds, formats and lints this
  repository, so `cargo fmt` on a contributor's machine gives what CI sees.
  `rust-version` in `Cargo.toml` is proven by the `msrv` job below rather than by
  the toolchain CI happens to run.
- CI restores the pinned toolchain from a cache keyed on `rust-toolchain.toml`,
  before the first rustup call in each job. Pinning made every job fetch and
  unpack the toolchain again, and that cost is single-threaded xz rather than
  bandwidth, so a faster runner does not pay it back.
- An `msrv` job builds the crate with the `rust-version` read out of
  `Cargo.toml`, so bumping that one line moves what CI actually walks.
- CI refuses a pull request into `main` that does not come from `develop`, so a
  release is always cut from the branch it was integrated on.

### Changed

- The explorer's detail pane opens and closes with `SPC v d`, not `SPC v p`, so
  that the key toggling it and the key placing it (`SPC o d`) read as one pair.
  **`SPC v d` used to show and hide the `done` column**, so that keystroke now
  does something else.
- `done` has no letter of its own under `SPC v` any more — `SPC v d` and the
  column's number toggled the same setting, and the menu said it twice. The
  number alone counts, and it reads the column's name out of the config instead
  of carrying `done` in the code.
- `moai` exits non-zero and names the file when a journal cannot be read. A skip
  drew a history that had quietly lost one person's lines and still looked whole.
- `moai init` no longer creates an empty `.moai/journal.jsonl`. History lives in
  `.moai/journal/` now, and the old path is only ever read.

### Fixed

- The release workflow could not have built its musl target. `ureq` pulls in
  `rustls`, which pulls in `ring`, which builds C and assembly, and that job had
  no C toolchain — `cc` resolves `x86_64-unknown-linux-musl` to
  `x86_64-linux-musl-gcc` or `musl-gcc` and `ubuntu-latest` carries neither. The
  job would have died *after* the tag was pushed, and `release` needs `build`, so
  nothing at all would have shipped. No pull request could have caught it:
  `ci.yml` and `smoke.yml` build the host target only.
- The pre-push tag check read the working tree's `Cargo.toml` instead of the
  `Cargo.toml` of the commit being pushed, so on a branch already moved on to the
  next version it refused to re-push an older tag whose version was correct.
  Shallow clones and machines without git fall back to the working tree, and the
  line it prints says which of the two it read.
- `release.yml` restored its cache after `rustup target add` — the first rustup
  call in that job — so the toolchain was already downloaded by the time the
  cache arrived.
- The install one-liner in `README.md` and `install.sh` points at `main`, the
  branch a release is cut from. It pointed at `develop`, so a receiver installed
  a release with a script that release does not carry.
- A journal file that cannot be read no longer blocks the rest of the history.
  The run steps over it, counts what it skipped, and names the file.
- Read marks survive their own edges: a line that cannot be read no longer stops
  the whole write, a stamp for a place that is gone is kept as pending instead of
  dropped, stamps already seated are taken back out of the pending list, `path`
  no longer swallows the comment above it, and a run that stops on a pending
  place says which file it stopped on.
- The explorer no longer lets a sweep that arrives late overwrite a row just
  opened, and the read marks of an expanded project follow a table that changed
  under them.
- A checkout with no tracker in it is told to run `moai init` first, and `init`
  no longer tells you to ignore the tracker file it is reading.
- The hook that `moai skill install` plants reads more shells correctly: heredoc
  bodies, `su -c` and `runuser -c` text, `runuser -u <user> -- <command>`,
  commands behind `xargs`, the `errexit` spellings zsh and ksh use, nested `!`
  where the outer one wins, a `-C` that points where you already are, and a
  pick-up that wins inside text that ends in a background `&`. Its re-reading of
  quoted text is bounded by a budget rather than by depth alone, and the same
  fragment is no longer scanned twice in one run.

## [0.1.0] - 2026-09-21

### Added

- Issues, epics, milestones, ideas and deferral, stored as one JSONL line per
  issue with a separate journal that is never read to compute state.
- `moai status`, `moai ready`, `moai show`, and a terminal explorer (`moai tui`).
- `moai prime`, a short markdown page of what you hold and what comes next —
  small enough for an agent to re-read at the start of a session and again
  after a compaction.
- `--json` on every command, so an agent loop can run with no human in it.
- `moai mv <id> in_progress --from todo` moves only while the column you saw
  still holds, so sessions sharing one repository cannot overwrite each other's
  pick-up. The loser gets `stale` and a non-zero exit code.
- `moai mv <id> done` says in one line what that close unblocked.
- `moai link` to record that one issue blocks another, and `moai read` to mark
  an issue read, kept in your own config only.
- `moai add --from -` creates an epic and its issues in one go from a plan
  written in markdown; `moai idea promote` unfolds a parked thought the same way.
- While a milestone is running, `moai ready` hands out what is inside it first
  and leaves only `p0` outside it.
- Commits are found on the spot by the issue id written in their subject —
  `moai show <id>` and the explorer draw them, and nothing is stored on the issue.
- `moai show <id> --json` reads the `model:` notes back as `work`, and carries
  `started_at` and `done_at` for how long a piece of work took.
- English is the language on screen by default. `MOAI_LANG=ko moai status`, or
  `lang` under `[i18n]` in your user config, switches it; en, ko, zh, ja and es
  exist, en and ko are complete, and the system locale is never read.
- `moai init`, which writes `.moai/` and a managed block in `AGENTS.md`.
- `moai merge-driver --install`, resolving `.moai/issues.jsonl` per issue
  instead of per neighbouring line. `moai status` measures whether the driver
  actually runs and shines one line on a clone where it was never installed.
- `moai skill install`, which installs the Claude skills and hooks.
- A `prepare-commit-msg` hook that writes `MOAI_ACTOR` into an `Executed-By:`
  trailer, so `git log` alone shows which commits an agent made.
- Multiple projects registered in one place, and worktree-aware `ready`.
- CI: clippy, `cargo fmt --check` against a committed `rustfmt.toml`, and tests
  on every pull request, plus a 15 MB release binary budget.
- A release workflow that publishes verified archives on a `v*` tag, and
  `install.sh`, which refuses to install anything it cannot check against
  `SHA256SUMS`.
